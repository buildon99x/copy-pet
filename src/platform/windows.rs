//! Native Win32 backend: a per-pixel-alpha layered window (transparent pixels
//! are click-through), low-level keyboard/mouse hooks feeding `crate::input`,
//! and a Shell notification-area (tray) icon with a context menu. This is the
//! release target on Windows; the cross-platform simulation lives in
//! [`crate::pet::Pet`].

use crate::input;
use crate::pet::{window_size, Pet, SCALES};
use crate::render;
use crate::state::{Persist, ACCESSORIES};
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_TRAY: u32 = WM_APP + 1;
// WM_MOUSELEAVE lives in Win32::UI::Controls in windows-sys; define it locally.
const WM_MOUSELEAVE: u32 = 0x02A3;
const TIMER_ID: usize = 1;
const TICK_MS: u32 = 33;

// menu command ids
const CMD_BUBBLE: usize = 10;
const CMD_LOCK: usize = 11;
const CMD_AUTOSTART: usize = 12;
const CMD_RESET: usize = 13;
const CMD_ABOUT: usize = 14;
const CMD_EXIT: usize = 15;
const CMD_SIZE0: usize = 20; // ..=22
const CMD_SOUND0: usize = 30; // ..=32
const CMD_ACC0: usize = 40; // 40 = none, 41..=46 accessories

fn wz(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---- app state -------------------------------------------------------------

struct App {
    hwnd: HWND,
    // surface
    mem_dc: HDC,
    dib: HBITMAP,
    bits: *mut u8,
    w: i32,
    h: i32,
    pm: tiny_skia::Pixmap,
    // simulation
    pet: Pet,
    // interaction
    mouse_down: bool,
    drag_moved: bool,
    drag_cursor: POINT,
    drag_win: (i32, i32),
    hover_tracking: bool,
    visible: bool,
    // win handles
    kbd_hook: HHOOK,
    mouse_hook: HHOOK,
    icon: HICON,
    taskbar_created: u32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` with the app if it exists and is not already borrowed
/// (re-entrant wndproc calls during modal loops simply skip).
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            guard.as_mut().map(f)
        } else {
            None
        }
    })
}

impl App {
    fn window_pos(&self) -> (i32, i32) {
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe { GetWindowRect(self.hwnd, &mut rc) };
        (rc.left, rc.top)
    }

    // ---- per-frame update --------------------------------------------------

    fn tick(&mut self) {
        let (k, c, wh) = input::drain();
        let redraw = self.pet.advance(k, c, wh);

        if self.pet.take_level_changed() {
            self.update_tray_tip();
        }
        if self.pet.should_autosave() {
            let (x, y) = self.window_pos();
            self.pet.save_pos(x, y);
        }
        if self.visible && redraw {
            self.pet.render(&mut self.pm);
            self.blit();
        }
    }

    // ---- surface / blit ----------------------------------------------------

    fn blit(&mut self) {
        let data = self.pm.data();
        let len = (self.w * self.h * 4) as usize;
        unsafe {
            let dst = std::slice::from_raw_parts_mut(self.bits, len);
            // tiny-skia: premultiplied RGBA -> GDI wants premultiplied BGRA
            for (d, s) in dst.chunks_exact_mut(4).zip(data.chunks_exact(4)) {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = s[3];
            }
            let screen = GetDC(null_mut());
            let size = SIZE {
                cx: self.w,
                cy: self.h,
            };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                self.hwnd,
                screen,
                null(),
                &size,
                self.mem_dc,
                &src,
                0,
                &blend,
                ULW_ALPHA,
            );
            ReleaseDC(null_mut(), screen);
        }
    }

    fn resize_surface(&mut self) {
        let (w, h) = window_size(self.pet.scale());
        // keep bottom-center anchored
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe {
            GetWindowRect(self.hwnd, &mut rc);
        }
        let nx = rc.left + ((rc.right - rc.left) - w) / 2;
        let ny = rc.top + ((rc.bottom - rc.top) - h);
        unsafe {
            DeleteObject(self.dib as _);
            DeleteDC(self.mem_dc);
            let (dc, dib, bits) = create_surface(w, h);
            self.mem_dc = dc;
            self.dib = dib;
            self.bits = bits;
            self.w = w;
            self.h = h;
            self.pm = tiny_skia::Pixmap::new(w as u32, h as u32).unwrap();
            SetWindowPos(
                self.hwnd,
                null_mut(),
                nx,
                ny,
                w,
                h,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.pet.save_pos(nx, ny);
        self.tick();
    }

    fn update_tray_tip(&self) {
        unsafe {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_TIP;
            let tip = wz(&self.pet.tooltip());
            let n = tip.len().min(127);
            nid.szTip[..n].copy_from_slice(&tip[..n]);
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }
}

// ---- surface ----------------------------------------------------------------

unsafe fn create_surface(w: i32, h: i32) -> (HDC, HBITMAP, *mut u8) {
    let screen = GetDC(null_mut());
    let dc = CreateCompatibleDC(screen);
    ReleaseDC(null_mut(), screen);
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = w;
    bmi.bmiHeader.biHeight = -h; // top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut c_void = null_mut();
    let dib = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    SelectObject(dc, dib as _);
    (dc, dib, bits as *mut u8)
}

unsafe fn make_icon() -> HICON {
    let mut pm = tiny_skia::Pixmap::new(32, 32).unwrap();
    render::draw_icon(&mut pm);
    let screen = GetDC(null_mut());
    let dc = CreateCompatibleDC(screen);
    ReleaseDC(null_mut(), screen);
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = 32;
    bmi.bmiHeader.biHeight = -32;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut c_void = null_mut();
    let dib = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    if !bits.is_null() {
        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, 32 * 32 * 4);
        for (d, s) in dst.chunks_exact_mut(4).zip(pm.data().chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
    }
    let mask = CreateBitmap(32, 32, 1, 1, null());
    let ii = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask,
        hbmColor: dib,
    };
    let icon = CreateIconIndirect(&ii);
    DeleteObject(dib as _);
    DeleteObject(mask as _);
    DeleteDC(dc);
    icon
}

// ---- hooks -------------------------------------------------------------------

unsafe extern "system" fn kbd_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wp as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            input::key();
        }
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wp as u32;
        match msg {
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN => input::click(),
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => input::wheel(),
            _ => {}
        }
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}

// ---- tray ----------------------------------------------------------------------

unsafe fn tray_add(hwnd: HWND, icon: HICON) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon;
    let tip = wz("DeskCat");
    nid.szTip[..tip.len()].copy_from_slice(&tip);
    Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn tray_remove(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

// ---- autostart (HKCU Run key) ----------------------------------------------------

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "DeskCat";

unsafe fn autostart_enabled() -> bool {
    let mut hkey: HKEY = null_mut();
    let key = wz(RUN_KEY);
    if RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
        return false;
    }
    let val = wz(RUN_VALUE);
    let ok = RegQueryValueExW(hkey, val.as_ptr(), null_mut(), null_mut(), null_mut(), null_mut())
        == 0;
    RegCloseKey(hkey);
    ok
}

unsafe fn set_autostart(on: bool) {
    let mut hkey: HKEY = null_mut();
    let key = wz(RUN_KEY);
    if RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE, &mut hkey) != 0 {
        return;
    }
    let val = wz(RUN_VALUE);
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let path = wz(&format!("\"{}\"", exe.display()));
            RegSetValueExW(
                hkey,
                val.as_ptr(),
                0,
                REG_SZ,
                path.as_ptr() as *const u8,
                (path.len() * 2) as u32,
            );
        }
    } else {
        RegDeleteValueW(hkey, val.as_ptr());
    }
    RegCloseKey(hkey);
}

// ---- context menu -----------------------------------------------------------------

struct MenuSnapshot {
    bubble: bool,
    locked: bool,
    scale_idx: usize,
    sound: u8,
    accessory: usize,
    level: u32,
    autostart: bool,
}

unsafe fn show_menu(hwnd: HWND, ms: &MenuSnapshot) -> usize {
    let menu = CreatePopupMenu();

    let chk = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };

    AppendMenuW(
        menu,
        MF_STRING | chk(ms.bubble),
        CMD_BUBBLE,
        wz("통계 항상 표시").as_ptr(),
    );

    // size submenu
    let size_menu = CreatePopupMenu();
    for (i, name) in ["작게", "보통", "크게"].iter().enumerate() {
        AppendMenuW(
            size_menu,
            MF_STRING | chk(ms.scale_idx == i),
            CMD_SIZE0 + i,
            wz(name).as_ptr(),
        );
    }
    AppendMenuW(menu, MF_POPUP, size_menu as usize, wz("크기").as_ptr());

    // accessory submenu
    let acc_menu = CreatePopupMenu();
    AppendMenuW(
        acc_menu,
        MF_STRING | chk(ms.accessory == 0),
        CMD_ACC0,
        wz("없음").as_ptr(),
    );
    for (i, acc) in ACCESSORIES.iter().enumerate() {
        let unlocked = ms.level >= acc.level;
        let label = if unlocked {
            acc.name_kr.to_string()
        } else {
            format!("{} (LV {} 달성 시)", acc.name_kr, acc.level)
        };
        let mut flags = MF_STRING | chk(ms.accessory == i + 1);
        if !unlocked {
            flags |= MF_GRAYED;
        }
        AppendMenuW(acc_menu, flags, CMD_ACC0 + 1 + i, wz(&label).as_ptr());
    }
    AppendMenuW(menu, MF_POPUP, acc_menu as usize, wz("액세서리").as_ptr());

    // sound submenu
    let snd_menu = CreatePopupMenu();
    for (i, name) in ["끄기", "이벤트 소리만", "타이핑 소리 + 이벤트"]
        .iter()
        .enumerate()
    {
        AppendMenuW(
            snd_menu,
            MF_STRING | chk(ms.sound as usize == i),
            CMD_SOUND0 + i,
            wz(name).as_ptr(),
        );
    }
    AppendMenuW(menu, MF_POPUP, snd_menu as usize, wz("소리").as_ptr());

    AppendMenuW(
        menu,
        MF_STRING | chk(ms.locked),
        CMD_LOCK,
        wz("위치 잠금").as_ptr(),
    );
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(
        menu,
        MF_STRING | chk(ms.autostart),
        CMD_AUTOSTART,
        wz("Windows 시작 시 자동 실행").as_ptr(),
    );
    AppendMenuW(menu, MF_STRING, CMD_RESET, wz("통계 초기화...").as_ptr());
    AppendMenuW(menu, MF_STRING, CMD_ABOUT, wz("DeskCat 정보").as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(menu, MF_STRING, CMD_EXIT, wz("종료").as_ptr());

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
        pt.x,
        pt.y,
        0,
        hwnd,
        null(),
    );
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);
    cmd as usize
}

fn menu_snapshot() -> Option<MenuSnapshot> {
    with_app(|a| MenuSnapshot {
        bubble: a.pet.st.bubble_pinned,
        locked: a.pet.st.locked,
        scale_idx: a.pet.st.scale_idx,
        sound: a.pet.st.sound_mode,
        accessory: a.pet.st.accessory,
        level: a.pet.level(),
        autostart: unsafe { autostart_enabled() },
    })
}

unsafe fn open_menu(hwnd: HWND) {
    if let Some(ms) = menu_snapshot() {
        let cmd = show_menu(hwnd, &ms);
        match cmd {
            0 => {}
            CMD_EXIT => {
                DestroyWindow(hwnd);
            }
            CMD_ABOUT => {
                let (lv, keys) =
                    with_app(|a| (a.pet.level(), a.pet.st.total_keys)).unwrap_or((1, 0));
                let text = format!(
                    "DeskCat v{}\n\n키보드와 함께 자라는 데스크탑 고양이 🐱\n\n현재 레벨: LV {}\n누적 키 입력: {}\n\n• 타이핑/클릭 → 고양이가 따라 칩니다 (XP 획득)\n• 레벨업 → 새 액세서리 잠금해제\n• 더블클릭 → 쓰다듬기\n• 드래그 → 위치 이동\n• 마우스 올리기 → 오늘의 통계",
                    env!("CARGO_PKG_VERSION"),
                    lv,
                    keys
                );
                MessageBoxW(
                    hwnd,
                    wz(&text).as_ptr(),
                    wz("DeskCat 정보").as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            CMD_RESET => {
                let answer = MessageBoxW(
                    hwnd,
                    wz("모든 통계와 레벨을 초기화할까요?\n(되돌릴 수 없습니다)").as_ptr(),
                    wz("통계 초기화").as_ptr(),
                    MB_YESNO | MB_ICONWARNING,
                );
                if answer == IDYES {
                    with_app(|a| {
                        a.pet.reset_stats();
                        a.update_tray_tip();
                    });
                }
            }
            CMD_AUTOSTART => {
                set_autostart(!ms.autostart);
            }
            CMD_BUBBLE => {
                with_app(|a| {
                    a.pet.st.bubble_pinned = !a.pet.st.bubble_pinned;
                    a.pet.dirty = true;
                });
            }
            CMD_LOCK => {
                with_app(|a| {
                    a.pet.st.locked = !a.pet.st.locked;
                    a.pet.dirty = true;
                });
            }
            c if (CMD_SIZE0..CMD_SIZE0 + SCALES.len()).contains(&c) => {
                with_app(|a| {
                    a.pet.st.scale_idx = c - CMD_SIZE0;
                    a.resize_surface();
                });
            }
            c if (CMD_SOUND0..CMD_SOUND0 + 3).contains(&c) => {
                with_app(|a| {
                    a.pet.st.sound_mode = (c - CMD_SOUND0) as u8;
                    a.pet.dirty = true;
                });
            }
            c if (CMD_ACC0..=CMD_ACC0 + ACCESSORIES.len()).contains(&c) => {
                with_app(|a| {
                    a.pet.st.accessory = c - CMD_ACC0;
                    a.pet.dirty = true;
                });
            }
            _ => {}
        }
    }
}

// ---- wndproc -------------------------------------------------------------------------

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wp == TIMER_ID {
                with_app(|a| a.tick());
            }
            0
        }
        WM_LBUTTONDOWN => {
            with_app(|a| {
                a.mouse_down = true;
                a.drag_moved = false;
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                a.drag_cursor = pt;
                a.drag_win = a.window_pos();
                SetCapture(hwnd);
            });
            0
        }
        WM_MOUSEMOVE => {
            with_app(|a| {
                if !a.hover_tracking {
                    a.hover_tracking = true;
                    a.pet.set_hover(true);
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    TrackMouseEvent(&mut tme);
                }
                if a.mouse_down && !a.pet.st.locked {
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    let dx = pt.x - a.drag_cursor.x;
                    let dy = pt.y - a.drag_cursor.y;
                    if a.drag_moved || dx.abs() > 3 || dy.abs() > 3 {
                        a.drag_moved = true;
                        SetWindowPos(
                            hwnd,
                            null_mut(),
                            a.drag_win.0 + dx,
                            a.drag_win.1 + dy,
                            0,
                            0,
                            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                }
            });
            0
        }
        WM_MOUSELEAVE => {
            with_app(|a| {
                a.pet.set_hover(false);
                a.hover_tracking = false;
            });
            0
        }
        WM_LBUTTONUP => {
            with_app(|a| {
                if a.mouse_down {
                    a.mouse_down = false;
                    ReleaseCapture();
                    if a.drag_moved {
                        let (x, y) = a.window_pos();
                        a.pet.save_pos(x, y);
                    } else {
                        let scale = a.pet.scale();
                        let cx = (lp & 0xFFFF) as i16 as f32 / scale;
                        let cy = ((lp >> 16) & 0xFFFF) as i16 as f32 / scale;
                        a.pet.click_bounce(cx, cy);
                    }
                }
            });
            0
        }
        WM_LBUTTONDBLCLK => {
            with_app(|a| {
                a.mouse_down = false;
                a.pet.pet();
            });
            0
        }
        WM_RBUTTONUP => {
            open_menu(hwnd);
            0
        }
        WM_TRAY => {
            let ev = (lp & 0xFFFF) as u32;
            match ev {
                WM_LBUTTONUP => {
                    with_app(|a| {
                        a.visible = !a.visible;
                        ShowWindow(hwnd, if a.visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
                    });
                }
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    open_menu(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_DISPLAYCHANGE => {
            with_app(|a| {
                let (px, py) = a.window_pos();
                let (x, y) = clamp_to_screen(px, py, a.w, a.h);
                if x != px || y != py {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        x,
                        y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            });
            0
        }
        WM_QUERYENDSESSION => {
            with_app(|a| {
                let (x, y) = a.window_pos();
                a.pet.save_pos(x, y);
            });
            1
        }
        WM_DESTROY => {
            with_app(|a| {
                let (x, y) = a.window_pos();
                a.pet.save_pos(x, y);
                KillTimer(hwnd, TIMER_ID);
                if !a.kbd_hook.is_null() {
                    UnhookWindowsHookEx(a.kbd_hook);
                }
                if !a.mouse_hook.is_null() {
                    UnhookWindowsHookEx(a.mouse_hook);
                }
                tray_remove(hwnd);
            });
            PostQuitMessage(0);
            0
        }
        _ => {
            // explorer restarted: re-add tray icon
            let restored = with_app(|a| {
                if msg == a.taskbar_created && msg != 0 {
                    tray_add(hwnd, a.icon);
                    a.update_tray_tip();
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
            if restored {
                0
            } else {
                DefWindowProcW(hwnd, msg, wp, lp)
            }
        }
    }
}

fn clamp_to_screen(x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
    unsafe {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let nx = x.clamp(vx - w + 60, vx + vw - 60);
        let ny = y.clamp(vy - h + 60, vy + vh - 60);
        (nx, ny)
    }
}

// ---- entry ------------------------------------------------------------------------------

pub fn run() {
    unsafe {
        // single instance
        CreateMutexW(null(), 0, wz("Local\\DeskCat-Singleton").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        SetProcessDpiAwarenessContext(-4 as _); // PER_MONITOR_AWARE_V2
        crate::sound::init();

        let hinst = GetModuleHandleW(null());
        let icon = make_icon();
        let class_name = wz("DeskCatWindow");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: icon,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: icon,
        };
        RegisterClassExW(&wc);

        let st = Persist::load();
        let scale = SCALES[st.scale_idx.min(2)];
        let (w, h) = window_size(scale);

        // default: bottom-right of the primary work area, desk on the taskbar
        let (mut x, mut y) = if st.has_pos {
            (st.pos_x, st.pos_y)
        } else {
            let mut wa = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            };
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut wa as *mut _ as *mut c_void, 0);
            (wa.right - w - 28, wa.bottom - h + (h as f32 * 0.05) as i32)
        };
        let (cx, cy) = clamp_to_screen(x, y, w, h);
        x = cx;
        y = cy;

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            wz("DeskCat").as_ptr(),
            WS_POPUP,
            x,
            y,
            w,
            h,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        if hwnd.is_null() {
            return;
        }

        let (mem_dc, dib, bits) = create_surface(w, h);

        let app = App {
            hwnd,
            mem_dc,
            dib,
            bits,
            w,
            h,
            pm: tiny_skia::Pixmap::new(w as u32, h as u32).unwrap(),
            pet: Pet::new(st),
            mouse_down: false,
            drag_moved: false,
            drag_cursor: POINT { x: 0, y: 0 },
            drag_win: (0, 0),
            hover_tracking: false,
            visible: true,
            kbd_hook: SetWindowsHookExW(WH_KEYBOARD_LL, Some(kbd_hook), null_mut(), 0),
            mouse_hook: SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), null_mut(), 0),
            icon,
            taskbar_created: RegisterWindowMessageW(wz("TaskbarCreated").as_ptr()),
        };

        APP.with(|cell| *cell.borrow_mut() = Some(app));

        tray_add(hwnd, icon);
        with_app(|a| a.update_tray_tip());
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        with_app(|a| {
            a.pet.render(&mut a.pm);
            a.blit();
        }); // first paint
        SetTimer(hwnd, TIMER_ID, TICK_MS, None);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
