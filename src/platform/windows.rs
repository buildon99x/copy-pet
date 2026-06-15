//! Native Win32 backend: a per-pixel-alpha layered window (transparent pixels
//! are click-through), low-level keyboard/mouse hooks feeding `crate::input`,
//! a clipboard-format listener feeding `crate::clipboard`, a global panel
//! hotkey (Win+Shift+V by default, configurable — see [`crate::hotkey`]),
//! and a Shell notification-area (tray) icon with a localized context menu.
//! This is the release target on Windows; the cross-platform simulation
//! lives in [`crate::pet::Pet`].
//!
//! While the clipboard panel is open the window temporarily drops
//! `WS_EX_NOACTIVATE` and takes focus so the search box can receive
//! keyboard input (incl. IME-composed Hangul via WM_CHAR).

use crate::clipboard::{b64_decode, b64_encode, RichFormats, MAX_RICH};
use crate::hotkey::{self, Hotkey};
use crate::i18n::{self, t, Lang, Msg};
use crate::input;
use crate::panel::{fit_delta, NavKey, Rect};
use crate::pet::{window_size, ClipPick, Pet, SCALES};
use crate::render::{self, Badge};
use crate::state::{Persist, ACCESSORIES};
use crate::update;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
    GetClipboardOwner, OpenClipboard, RegisterClipboardFormatW, RemoveClipboardFormatListener,
    SetClipboardData,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, CreateMutexW, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW,
    Sleep, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, RegisterHotKey, ReleaseCapture, SendInput, SetCapture, SetFocus, TrackMouseEvent,
    UnregisterHotKey, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN, TME_LEAVE, TRACKMOUSEEVENT, VK_CONTROL,
};
use windows_sys::Win32::UI::Shell::{
    ExtractIconExW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_TRAY: u32 = WM_APP + 1;
// WM_MOUSELEAVE lives in Win32::UI::Controls in windows-sys; define it locally.
const WM_MOUSELEAVE: u32 = 0x02A3;
/// Standard clipboard format CF_UNICODETEXT (avoids the Win32_System_Ole
/// feature for one constant).
const CF_UNICODETEXT: u32 = 13;

/// Returns the registered format id for `"HTML Format"` (lazy, thread-safe).
fn cf_html_id() -> u32 {
    static ID: OnceLock<u32> = OnceLock::new();
    *ID.get_or_init(|| {
        let n = wz("HTML Format");
        unsafe { RegisterClipboardFormatW(n.as_ptr()) }
    })
}

/// Returns the registered format id for `"Rich Text Format"` (lazy, thread-safe).
fn cf_rtf_id() -> u32 {
    static ID: OnceLock<u32> = OnceLock::new();
    *ID.get_or_init(|| {
        let n = wz("Rich Text Format");
        unsafe { RegisterClipboardFormatW(n.as_ptr()) }
    })
}
const TIMER_ID: usize = 1;
const TICK_MS: u32 = 33;
const HOTKEY_ID: i32 = 1;

// menu command ids
const CMD_BUBBLE: usize = 10;
const CMD_LOCK: usize = 11;
const CMD_AUTOSTART: usize = 12;
const CMD_RESET: usize = 13;
const CMD_ABOUT: usize = 14;
const CMD_EXIT: usize = 15;
const CMD_PANEL: usize = 16;
const CMD_CAPTURE: usize = 17;
const CMD_UPDATE: usize = 18;
const CMD_AUTOUPDATE: usize = 19;
const CMD_AUTOCLOSE: usize = 23;
const CMD_GITHUB: usize = 24;
const CMD_SIZE0: usize = 20; // ..=22
const CMD_SOUND0: usize = 30; // ..=32
const CMD_ACC0: usize = 40; // 40 = none, 41..=46 accessories
const CMD_LANG_EN: usize = 50;
const CMD_LANG_KO: usize = 51;
const CMD_WINLEVEL0: usize = 52; // 52 top, 53 normal, 54 hide
const CMD_HOTKEY: usize = 55;

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
    // clipboard
    suppress_clip: Option<String>,
    // interaction
    mouse_down: bool,
    drag_moved: bool,
    /// A panel-card drag (header move / grip resize) is in progress; the
    /// cursor delta is fed to `Pet::panel_drag_update` in screen pixels
    /// because the window itself moves/resizes under the cursor.
    panel_drag: bool,
    drag_cursor: POINT,
    drag_win: (i32, i32),
    hover_tracking: bool,
    visible: bool,
    pending_surrogate: Option<u16>,
    // win handles
    kbd_hook: HHOOK,
    mouse_hook: HHOOK,
    icon: HICON,
    taskbar_created: u32,
    /// Display label of the actually registered panel hotkey (tray menu,
    /// About dialog); empty when no hotkey could be registered.
    hotkey_label: String,
    /// The window that had focus when the panel was opened — where auto-paste
    /// (`paste_on_select`) sends the synthesized Ctrl+V. Null until captured.
    paste_target: HWND,
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

    /// Client coords (from an lParam) in physical pixels. The panel is
    /// hit-tested at scale 1.0 (its coords are physical) and `Pet::cat_point`
    /// divides by the cat scale itself, so neither needs pre-division.
    fn client_xy(&self, lp: LPARAM) -> (f32, f32) {
        let cx = (lp & 0xFFFF) as i16 as f32;
        let cy = ((lp >> 16) & 0xFFFF) as i16 as f32;
        (cx, cy)
    }

    /// Puts panel-picked text on the OS clipboard; our own change is
    /// suppressed once in WM_CLIPBOARDUPDATE. When `pick.paste` (the
    /// `paste_on_select` setting), it then restores focus to the app that was
    /// active when the panel opened and synthesizes Ctrl+V there.
    fn copy_back(&mut self, pick: ClipPick) {
        self.suppress_clip = Some(pick.text.clone());
        unsafe { set_clipboard_pick(self.hwnd, &pick) };
        if pick.paste && !self.paste_target.is_null() && self.paste_target != self.hwnd {
            unsafe {
                // SetForegroundWindow is asynchronous and won't promote a
                // window owned by another thread on demand, so a bare
                // SetForegroundWindow + SendInput races: the Ctrl+V fires
                // before focus actually lands on the target and is lost. Attach
                // our input queue to the target thread first — that
                // synchronizes the foreground/focus state across both threads,
                // so the foreground switch takes effect before we synthesize
                // the keystroke. Always detach afterwards.
                let target = self.paste_target;
                let tid = GetWindowThreadProcessId(target, null_mut());
                let me = GetCurrentThreadId();
                let attached = tid != 0 && tid != me && AttachThreadInput(me, tid, 1) != 0;
                SetForegroundWindow(target);
                send_ctrl_v();
                if attached {
                    AttachThreadInput(me, tid, 0);
                }
            }
        }
    }

    // ---- per-frame update --------------------------------------------------

    /// One ~33 ms frame. Returns `true` when an installed update is in
    /// place and the app should relaunch (handled by the WM_TIMER caller).
    fn tick(&mut self) -> bool {
        let (k, c, wh) = input::drain();
        let redraw = self.pet.advance(k, c, wh);

        // auto-update: surface a found release / a finished install
        if let Some(v) = update::take_found() {
            self.pet.notify_update(&v);
        }
        match update::poll_install() {
            update::Install::Ready => {
                let (x, y) = self.window_pos();
                self.pet.save_pos(x, y);
                return true;
            }
            update::Install::Failed => self.pet.notify_update_failed(),
            _ => {}
        }

        if self.pet.take_level_changed() {
            self.update_tray_tip();
        }
        if self.pet.take_size_changed() {
            unsafe { self.apply_size() };
        }
        if self.pet.should_autosave() {
            let (x, y) = self.window_pos();
            self.pet.save_pos(x, y);
        }
        if self.visible && redraw {
            self.pet.render(&mut self.pm);
            self.blit();
        }
        false
    }

    // ---- surface / blit ----------------------------------------------------

    fn blit(&mut self) {
        self.blit_at(None);
    }

    /// Pushes the pixmap to the layered window. `move_to` additionally
    /// repositions the (possibly resized) window in the same
    /// UpdateLayeredWindow call — position, size and content change
    /// atomically, so a panel drag can never show a half-updated frame
    /// (the cat would visibly tremble otherwise).
    fn blit_at(&mut self, move_to: Option<(i32, i32)>) {
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
            let dst_pt = move_to.map(|(x, y)| POINT { x, y });
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                self.hwnd,
                screen,
                dst_pt.as_ref().map_or(null(), |p| p),
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

    /// Resizes window + surface to the pet's wanted size (scale or panel
    /// layout changed), shifted by `Pet::take_window_shift` so the cat stays
    /// put on screen, and adjusts focusability: the panel needs keyboard
    /// focus, the plain pet must never steal it. Move + resize + repaint go
    /// out as one atomic `UpdateLayeredWindow` (no flicker during drags).
    unsafe fn apply_size(&mut self) {
        let fit = self.pet.take_fit_panel();
        let (w, h) = self.pet.canvas_size();
        let (dx, dy) = self.pet.take_window_shift();
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetWindowRect(self.hwnd, &mut rc);
        let (mut nx, mut ny) = (rc.left + dx, rc.top + dy);
        if !self.pet.panel_open() {
            // shrunk back to the cat: keep it reachable. While the panel is
            // open the window legitimately extends offscreen (the card can
            // sit anywhere); clamping then would drag the cat along.
            (nx, ny) = clamp_to_screen(nx, ny, w, h);
        }
        if (w, h) != (self.w, self.h) {
            DeleteObject(self.dib as _);
            DeleteDC(self.mem_dc);
            let (dc, dib, bits) = create_surface(w, h);
            self.mem_dc = dc;
            self.dib = dib;
            self.bits = bits;
            self.w = w;
            self.h = h;
            self.pm = tiny_skia::Pixmap::new(w as u32, h as u32).unwrap();
        }

        // toggle focusability only on a real open/close transition — a panel
        // drag re-applies the size many times per second
        let ex = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
        let no_activate = ex & WS_EX_NOACTIVATE as isize != 0;
        if self.pet.panel_open() && no_activate {
            // remember who had focus before we steal it, so auto-paste can
            // send the clip back to that app
            self.paste_target = GetForegroundWindow();
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex & !(WS_EX_NOACTIVATE as isize));
            SetForegroundWindow(self.hwnd);
            SetFocus(self.hwnd);
        } else if !self.pet.panel_open() && !no_activate {
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE as isize);
        }

        // remember the position without forcing a disk write per drag step;
        // the throttled autosave (or exit) persists it
        self.pet.st.pos_x = nx;
        self.pet.st.pos_y = ny;
        self.pet.st.has_pos = true;
        self.pet.dirty = true;
        self.pet.render(&mut self.pm);
        self.blit_at(Some((nx, ny)));

        // The panel just opened: if the card landed off the monitor (the pet
        // sits near an edge, or a persisted offset put it offscreen), slide
        // the card — not the cat — back into view and re-apply once. `fit` is
        // already drained, so the recursive call can't loop.
        if fit && self.pet.panel_open() {
            if let Some((sdx, sdy)) = self.panel_fit_shift(nx, ny) {
                self.pet.shift_panel(sdx, sdy);
                self.apply_size();
            }
        }
    }

    /// Shift (physical pixels) that brings the open panel card fully onto the
    /// work area of the monitor it sits on, or None when it already fits.
    /// `(nx, ny)` is the window's top-left in screen pixels. The card geometry
    /// is already physical (it renders at scale 1.0), so no scale conversion.
    unsafe fn panel_fit_shift(&self, nx: i32, ny: i32) -> Option<(f32, f32)> {
        let l = self.pet.panel.layout();
        let card = Rect {
            x: nx as f32 + l.card_x,
            y: ny as f32 + l.card_y,
            w: l.card_w,
            h: l.card_h,
        };
        let (dx, dy) = fit_delta(card, monitor_work_rect(self.hwnd)?);
        (dx.abs() >= 0.5 || dy.abs() >= 0.5).then_some((dx, dy))
    }

    /// Applies the persisted window level: 0 = always on top, 1 = normal (can
    /// go behind other windows), 2 = hidden (restored from the tray icon or
    /// the global panel hotkey). Keeps `self.visible` — which gates the
    /// per-tick blit — in sync.
    unsafe fn apply_window_level(&mut self) {
        if self.pet.window_level() == 2 {
            self.visible = false;
            ShowWindow(self.hwnd, SW_HIDE);
        } else {
            self.visible = true;
            let after = if self.pet.window_level() == 0 {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            };
            SetWindowPos(self.hwnd, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    /// Brings the (possibly obscured or hidden) window to the front and gives
    /// it focus — the panel hotkey's "reveal". Un-hides first; in Normal mode
    /// the window isn't topmost, so raise it above other windows too.
    unsafe fn reveal(&mut self) {
        if self.pet.show_window() {
            self.apply_window_level(); // was hidden -> show at its level
        } else if self.pet.window_level() == 1 {
            // Normal: raise above other (non-topmost) windows
            SetWindowPos(self.hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        SetForegroundWindow(self.hwnd);
        SetFocus(self.hwnd);
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

    /// A WM_CHAR while the panel is open: search input (UTF-16, possibly a
    /// surrogate pair from the IME).
    fn panel_char_utf16(&mut self, unit: u16) {
        if (0xD800..0xDC00).contains(&unit) {
            self.pending_surrogate = Some(unit);
            return;
        }
        let c = if (0xDC00..0xE000).contains(&unit) {
            let Some(high) = self.pending_surrogate.take() else {
                return;
            };
            let cp = 0x10000 + (((high as u32 - 0xD800) << 10) | (unit as u32 - 0xDC00));
            char::from_u32(cp)
        } else {
            self.pending_surrogate = None;
            char::from_u32(unit as u32)
        };
        if let Some(c) = c {
            self.pet.panel_char(c);
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

// ---- clipboard --------------------------------------------------------------

/// Reads CF_UNICODETEXT + CF_HTML + RTF from the clipboard in one open/close.
/// Retries briefly: the copying app may still hold the clipboard when
/// WM_CLIPBOARDUPDATE arrives.
/// SAFETY: handles from GetClipboardData are owned by the clipboard; we only
/// read between GlobalLock/GlobalUnlock and never free them.
unsafe fn read_clipboard_rich(hwnd: HWND) -> Option<(String, Option<RichFormats>)> {
    for attempt in 0..5 {
        if attempt > 0 {
            Sleep(15);
        }
        if OpenClipboard(hwnd) == 0 {
            continue;
        }
        // ---- plain text (required) ------------------------------------------
        let ht = GetClipboardData(CF_UNICODETEXT);
        let text_opt = if ht.is_null() {
            None
        } else {
            let p = GlobalLock(ht) as *const u16;
            if p.is_null() {
                None
            } else {
                let max = GlobalSize(ht) / 2;
                let mut len = 0usize;
                while len < max && *p.add(len) != 0 {
                    len += 1;
                }
                let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
                GlobalUnlock(ht);
                Some(s)
            }
        };
        // ---- HTML Format (optional, UTF-8 bytes) ----------------------------
        let html = read_bytes_format(cf_html_id()).and_then(|b| {
            // CF_HTML is null-terminated UTF-8; trim the trailing null if present
            let trimmed = b.split(|&c| c == 0).next().unwrap_or(&b);
            if trimmed.len() <= MAX_RICH {
                std::str::from_utf8(trimmed).ok().map(|s| s.to_string())
            } else {
                None
            }
        });
        // ---- Rich Text Format (optional, raw bytes → base64) ----------------
        let rtf_b64 = read_bytes_format(cf_rtf_id()).and_then(|b| {
            if b.len() <= MAX_RICH {
                Some(b64_encode(&b))
            } else {
                None
            }
        });
        CloseClipboard();
        let text = text_opt?;
        let formats = if html.is_some() || rtf_b64.is_some() {
            Some(RichFormats { html, rtf_b64 })
        } else {
            None
        };
        return Some((text, formats));
    }
    None
}

/// Reads raw bytes for a registered clipboard format without closing the clipboard.
/// SAFETY: clipboard must already be open; handle is owned by the clipboard.
unsafe fn read_bytes_format(fmt: u32) -> Option<Vec<u8>> {
    if fmt == 0 {
        return None;
    }
    let h = GetClipboardData(fmt);
    if h.is_null() {
        return None;
    }
    let p = GlobalLock(h) as *const u8;
    if p.is_null() {
        return None;
    }
    let len = GlobalSize(h);
    let bytes = std::slice::from_raw_parts(p, len).to_vec();
    GlobalUnlock(h);
    Some(bytes)
}

/// Writes bytes to a clipboard format (clipboard must already be open and empty).
/// Returns `true` on success; the clipboard takes ownership of the handle.
/// SAFETY: clipboard must be open; `GlobalAlloc`'d handle is transferred on success.
unsafe fn write_clip_bytes(fmt: u32, data: &[u8]) -> bool {
    if fmt == 0 || data.is_empty() {
        return false;
    }
    let h = GlobalAlloc(GMEM_MOVEABLE, data.len());
    if h.is_null() {
        return false;
    }
    let p = GlobalLock(h) as *mut u8;
    if p.is_null() {
        GlobalFree(h);
        return false;
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
    GlobalUnlock(h);
    let ok = !SetClipboardData(fmt, h).is_null();
    if !ok {
        GlobalFree(h);
    }
    ok
}

/// Replaces the clipboard with the given pick.  When `plain_only` is false and
/// `formats` is `Some`, also writes CF_HTML and/or RTF alongside the text so
/// the target app receives the original rich content.
/// SAFETY: GlobalAlloc'd buffers are transferred to the clipboard on success.
unsafe fn set_clipboard_pick(hwnd: HWND, pick: &ClipPick) -> bool {
    let wide: Vec<u16> = pick.text.encode_utf16().chain(std::iter::once(0)).collect();
    let text_bytes = wide.len() * 2;
    for attempt in 0..5 {
        if attempt > 0 {
            Sleep(15);
        }
        if OpenClipboard(hwnd) == 0 {
            continue;
        }
        EmptyClipboard();
        let wide_bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, text_bytes);
        let ok = write_clip_bytes(CF_UNICODETEXT, wide_bytes);
        if ok && !pick.plain_only {
            if let Some(fmt) = &pick.formats {
                if let Some(html) = &fmt.html {
                    // CF_HTML is UTF-8 with a null terminator
                    let mut html_bytes = html.as_bytes().to_vec();
                    html_bytes.push(0);
                    write_clip_bytes(cf_html_id(), &html_bytes);
                }
                if let Some(rtf_b64) = &fmt.rtf_b64 {
                    let rtf_bytes = b64_decode(rtf_b64);
                    write_clip_bytes(cf_rtf_id(), &rtf_bytes);
                }
            }
        }
        CloseClipboard();
        return ok;
    }
    false
}

/// Identifies the app that owns the clipboard (falling back to the
/// foreground window): short name + a fish badge with its real icon.
unsafe fn clipboard_source() -> (Option<String>, Option<Badge>) {
    let mut src = GetClipboardOwner();
    if src.is_null() {
        src = GetForegroundWindow();
    }
    if src.is_null() {
        return (None, None);
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(src, &mut pid);
    if pid == 0 {
        return (None, None);
    }
    let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if proc.is_null() {
        return (None, None);
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(proc, 0, buf.as_mut_ptr(), &mut len);
    CloseHandle(proc);
    if ok == 0 {
        return (None, None);
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    let name = path
        .rsplit(['\\', '/'])
        .next()
        .map(|n| n.trim_end_matches(".exe").trim_end_matches(".EXE").to_string())
        .filter(|n| !n.is_empty());

    let mut badge = Badge::from_source(name.as_deref());
    if let Some((size, rgba)) = extract_icon_rgba(&path) {
        badge.set_icon_rgba(size, &rgba);
    }
    (name, Some(badge))
}

/// Extracts the exe's small icon as straight-alpha RGBA pixels.
unsafe fn extract_icon_rgba(path: &str) -> Option<(u32, Vec<u8>)> {
    let wide = wz(path);
    let mut small: HICON = null_mut();
    let n = ExtractIconExW(wide.as_ptr(), 0, null_mut(), &mut small, 1);
    if n == 0 || small.is_null() {
        return None;
    }
    let rgba = icon_to_rgba(small);
    DestroyIcon(small);
    rgba
}

/// HICON -> (size, straight RGBA). Uses GetIconInfo + GetDIBits; icons
/// without an alpha channel fall back to the AND mask.
/// SAFETY: all GDI objects created here (the ICONINFO bitmaps) are deleted
/// before returning; buffers are sized from the queried bitmap dimensions.
unsafe fn icon_to_rgba(icon: HICON) -> Option<(u32, Vec<u8>)> {
    let mut ii: ICONINFO = std::mem::zeroed();
    if GetIconInfo(icon, &mut ii) == 0 {
        return None;
    }
    // Always delete both bitmaps on every path.
    struct Guard(HBITMAP, HBITMAP);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                if !self.0.is_null() {
                    DeleteObject(self.0 as _);
                }
                if !self.1.is_null() {
                    DeleteObject(self.1 as _);
                }
            }
        }
    }
    let guard = Guard(ii.hbmColor, ii.hbmMask);
    if ii.hbmColor.is_null() {
        return None; // monochrome icon: let the letter badge handle it
    }
    let mut bm: BITMAP = std::mem::zeroed();
    if GetObjectW(
        ii.hbmColor as _,
        std::mem::size_of::<BITMAP>() as i32,
        &mut bm as *mut _ as *mut c_void,
    ) == 0
    {
        return None;
    }
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    if !(8..=256).contains(&w) || !(8..=256).contains(&h) {
        return None;
    }

    let screen = GetDC(null_mut());
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = w;
    bmi.bmiHeader.biHeight = -h; // top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    let count = (w * h) as usize;
    let mut color = vec![0u8; count * 4];
    let got = GetDIBits(
        screen,
        ii.hbmColor,
        0,
        h as u32,
        color.as_mut_ptr() as *mut c_void,
        &mut bmi,
        DIB_RGB_COLORS,
    );
    let mut mask = vec![0u8; count * 4];
    let mut bmi2 = bmi;
    let got_mask = if ii.hbmMask.is_null() {
        0
    } else {
        GetDIBits(
            screen,
            ii.hbmMask,
            0,
            h as u32,
            mask.as_mut_ptr() as *mut c_void,
            &mut bmi2,
            DIB_RGB_COLORS,
        )
    };
    ReleaseDC(null_mut(), screen);
    drop(guard);
    if got == 0 {
        return None;
    }

    let has_alpha = color.chunks_exact(4).any(|p| p[3] != 0);
    let mut rgba = Vec::with_capacity(count * 4);
    for i in 0..count {
        let p = &color[i * 4..i * 4 + 4];
        let a = if has_alpha {
            p[3]
        } else if got_mask != 0 {
            // AND mask: white = transparent, black = opaque
            if mask[i * 4] == 0 {
                255
            } else {
                0
            }
        } else {
            255
        };
        rgba.extend_from_slice(&[p[2], p[1], p[0], a]); // BGRA -> RGBA
    }
    Some((w as u32, rgba))
}

// ---- panel hotkey -------------------------------------------------------------

/// Registers `hk` as the global panel hotkey. Best-effort: returns false on
/// a clash with another app's registration (or a combo Windows reserves).
unsafe fn register_hotkey(hwnd: HWND, hk: &Hotkey) -> bool {
    let mut mods = MOD_NOREPEAT;
    if hk.win {
        mods |= MOD_WIN;
    }
    if hk.ctrl {
        mods |= MOD_CONTROL;
    }
    if hk.shift {
        mods |= MOD_SHIFT;
    }
    if hk.alt {
        mods |= MOD_ALT;
    }
    // A-Z / 0-9 virtual-key codes equal their ASCII uppercase values
    RegisterHotKey(hwnd, HOTKEY_ID, mods, hk.key as u32) != 0
}

/// Outcome of trying to register the global panel hotkey.
enum HotkeyReg {
    /// The configured chord registered; carries its display label.
    Configured(String),
    /// The configured chord was unavailable (a clash, or a combo Windows
    /// reserves like Win+Shift+V); the Ctrl+Shift+V fallback registered
    /// instead. Carries the wanted and the used display labels.
    Fallback { wanted: String, used: String },
    /// Nothing registered (tray/middle-click still open the panel).
    None,
}

impl HotkeyReg {
    /// Display label of whichever chord stuck (empty when none did).
    fn label(&self) -> String {
        match self {
            HotkeyReg::Configured(s) | HotkeyReg::Fallback { used: s, .. } => s.clone(),
            HotkeyReg::None => String::new(),
        }
    }
}

/// Tries the configured hotkey, then the Ctrl+Shift+V fallback, reporting
/// which one stuck so the caller can flag a silent fallback to the user.
unsafe fn register_panel_hotkey(hwnd: HWND, spec: &str) -> HotkeyReg {
    let configured = Hotkey::from_spec(spec);
    if register_hotkey(hwnd, &configured) {
        return HotkeyReg::Configured(configured.display());
    }
    let fallback = Hotkey::from_spec(hotkey::FALLBACK);
    if fallback != configured && register_hotkey(hwnd, &fallback) {
        return HotkeyReg::Fallback {
            wanted: configured.display(),
            used: fallback.display(),
        };
    }
    HotkeyReg::None
}

/// Synthesizes a Ctrl+V keystroke into the foreground window (auto-paste).
/// Output-only: we never read keystrokes, so this does not touch the input
/// privacy guarantee (golden rule 1). The modifiers are released in reverse.
unsafe fn send_ctrl_v() {
    let mut inputs: [INPUT; 4] = std::mem::zeroed();
    for inp in &mut inputs {
        inp.r#type = INPUT_KEYBOARD;
    }
    const VK_V: u16 = 0x56;
    inputs[0].Anonymous.ki.wVk = VK_CONTROL; // Ctrl down
    inputs[1].Anonymous.ki.wVk = VK_V; // V down
    inputs[2].Anonymous.ki.wVk = VK_V; // V up
    inputs[2].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    inputs[3].Anonymous.ki.wVk = VK_CONTROL; // Ctrl up
    inputs[3].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        std::mem::size_of::<INPUT>() as i32,
    );
}

// ---- hooks -------------------------------------------------------------------

unsafe extern "system" fn kbd_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 {
        // The vkCode is reduced to a held/released bit so holding a key (OS
        // auto-repeat) counts once; it is never stored or inspected further
        // (see crate::input and golden rule 1).
        let vk = (*(lp as *const KBDLLHOOKSTRUCT)).vkCode as u16;
        match wp as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => input::key_down(vk),
            WM_KEYUP | WM_SYSKEYUP => input::key_up(vk),
            _ => {}
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
    let tip = wz("ClipCat");
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
const RUN_VALUE: &str = "ClipCat";
const RUN_VALUE_LEGACY: &str = "DeskCat";

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

/// Pre-2.0 installs registered autostart as "DeskCat"; carry it over once.
unsafe fn migrate_autostart() {
    let mut hkey: HKEY = null_mut();
    let key = wz(RUN_KEY);
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        key.as_ptr(),
        0,
        KEY_QUERY_VALUE | KEY_SET_VALUE,
        &mut hkey,
    ) != 0
    {
        return;
    }
    let legacy = wz(RUN_VALUE_LEGACY);
    let had_legacy =
        RegQueryValueExW(hkey, legacy.as_ptr(), null_mut(), null_mut(), null_mut(), null_mut())
            == 0;
    if had_legacy {
        RegDeleteValueW(hkey, legacy.as_ptr());
    }
    RegCloseKey(hkey);
    if had_legacy {
        set_autostart(true);
    }
}

// ---- context menu -----------------------------------------------------------------

struct MenuSnapshot {
    bubble: bool,
    locked: bool,
    scale_idx: usize,
    sound: u8,
    window_level: u8,
    accessory: usize,
    level: u32,
    autostart: bool,
    lang: Lang,
    capture: bool,
    autoclose: bool,
    panel_open: bool,
    hotkey_label: String,
    auto_update: bool,
    /// Newer release the user can install, when the checker found one.
    update: Option<String>,
}

unsafe fn show_menu(hwnd: HWND, ms: &MenuSnapshot) -> usize {
    let menu = CreatePopupMenu();
    let lang = ms.lang;

    let chk = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };

    // a found update goes on top, where it can't be missed
    if let Some(ver) = &ms.update {
        AppendMenuW(
            menu,
            MF_STRING,
            CMD_UPDATE,
            wz(&i18n::menu_update(lang, ver)).as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
    }

    AppendMenuW(
        menu,
        MF_STRING | chk(ms.panel_open),
        CMD_PANEL,
        wz(&i18n::menu_clipboard(lang, &ms.hotkey_label)).as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING | chk(!ms.capture),
        CMD_CAPTURE,
        wz(t(lang, Msg::MenuCapturePause)).as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING | chk(ms.autoclose),
        CMD_AUTOCLOSE,
        wz(t(lang, Msg::MenuAutoClose)).as_ptr(),
    );
    // Panel hotkey: one click cycles to the next preset (no rebind dialog).
    AppendMenuW(
        menu,
        MF_STRING,
        CMD_HOTKEY,
        wz(&i18n::menu_hotkey(lang, &ms.hotkey_label)).as_ptr(),
    );
    AppendMenuW(menu, MF_SEPARATOR, 0, null());

    AppendMenuW(
        menu,
        MF_STRING | chk(ms.bubble),
        CMD_BUBBLE,
        wz(t(lang, Msg::MenuShowStats)).as_ptr(),
    );

    // size submenu
    let size_menu = CreatePopupMenu();
    for (i, name) in [Msg::SizeSmall, Msg::SizeNormal, Msg::SizeLarge]
        .iter()
        .enumerate()
    {
        AppendMenuW(
            size_menu,
            MF_STRING | chk(ms.scale_idx == i),
            CMD_SIZE0 + i,
            wz(t(lang, *name)).as_ptr(),
        );
    }
    AppendMenuW(
        menu,
        MF_POPUP,
        size_menu as usize,
        wz(t(lang, Msg::MenuSize)).as_ptr(),
    );

    // accessory submenu
    let acc_menu = CreatePopupMenu();
    AppendMenuW(
        acc_menu,
        MF_STRING | chk(ms.accessory == 0),
        CMD_ACC0,
        wz(t(lang, Msg::AccNone)).as_ptr(),
    );
    for (i, acc) in ACCESSORIES.iter().enumerate() {
        let unlocked = ms.level >= acc.level;
        let label = if unlocked {
            acc.name(lang).to_string()
        } else {
            i18n::accessory_locked(lang, acc.name(lang), acc.level)
        };
        let mut flags = MF_STRING | chk(ms.accessory == i + 1);
        if !unlocked {
            flags |= MF_GRAYED;
        }
        AppendMenuW(acc_menu, flags, CMD_ACC0 + 1 + i, wz(&label).as_ptr());
    }
    AppendMenuW(
        menu,
        MF_POPUP,
        acc_menu as usize,
        wz(t(lang, Msg::MenuAccessory)).as_ptr(),
    );

    // sound submenu
    let snd_menu = CreatePopupMenu();
    for (i, name) in [Msg::SoundOff, Msg::SoundEvents, Msg::SoundAll]
        .iter()
        .enumerate()
    {
        AppendMenuW(
            snd_menu,
            MF_STRING | chk(ms.sound as usize == i),
            CMD_SOUND0 + i,
            wz(t(lang, *name)).as_ptr(),
        );
    }
    AppendMenuW(
        menu,
        MF_POPUP,
        snd_menu as usize,
        wz(t(lang, Msg::MenuSound)).as_ptr(),
    );

    // window stacking submenu (always on top / normal / hide)
    let win_menu = CreatePopupMenu();
    for (i, name) in [Msg::WinLevelTop, Msg::WinLevelNormal, Msg::WinLevelHide]
        .iter()
        .enumerate()
    {
        AppendMenuW(
            win_menu,
            MF_STRING | chk(ms.window_level as usize == i),
            CMD_WINLEVEL0 + i,
            wz(t(lang, *name)).as_ptr(),
        );
    }
    AppendMenuW(
        menu,
        MF_POPUP,
        win_menu as usize,
        wz(t(lang, Msg::MenuWindowLevel)).as_ptr(),
    );

    AppendMenuW(
        menu,
        MF_STRING | chk(ms.locked),
        CMD_LOCK,
        wz(t(lang, Msg::MenuLock)).as_ptr(),
    );

    // language submenu
    let lang_menu = CreatePopupMenu();
    AppendMenuW(
        lang_menu,
        MF_STRING | chk(lang == Lang::En),
        CMD_LANG_EN,
        wz("English").as_ptr(),
    );
    AppendMenuW(
        lang_menu,
        MF_STRING | chk(lang == Lang::Ko),
        CMD_LANG_KO,
        wz("한국어").as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_POPUP,
        lang_menu as usize,
        wz(t(lang, Msg::MenuLanguage)).as_ptr(),
    );

    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(
        menu,
        MF_STRING | chk(ms.autostart),
        CMD_AUTOSTART,
        wz(t(lang, Msg::MenuAutostart)).as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING | chk(ms.auto_update),
        CMD_AUTOUPDATE,
        wz(t(lang, Msg::MenuAutoUpdate)).as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING,
        CMD_RESET,
        wz(t(lang, Msg::MenuReset)).as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING,
        CMD_ABOUT,
        wz(t(lang, Msg::MenuAbout)).as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING,
        CMD_GITHUB,
        wz(t(lang, Msg::MenuGithub)).as_ptr(),
    );
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(
        menu,
        MF_STRING,
        CMD_EXIT,
        wz(t(lang, Msg::MenuExit)).as_ptr(),
    );

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
        window_level: a.pet.window_level(),
        accessory: a.pet.st.accessory,
        level: a.pet.level(),
        autostart: unsafe { autostart_enabled() },
        lang: a.pet.lang(),
        capture: a.pet.st.clip_capture,
        autoclose: a.pet.st.panel_autoclose,
        panel_open: a.pet.panel_open(),
        hotkey_label: a.hotkey_label.clone(),
        auto_update: a.pet.st.auto_update,
        update: a.pet.update_available().map(str::to_string),
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
                let (lv, keys, clips) =
                    with_app(|a| (a.pet.level(), a.pet.st.total_keys, a.pet.clips.len()))
                        .unwrap_or((1, 0, 0));
                let text = i18n::about_text(
                    ms.lang,
                    env!("CARGO_PKG_VERSION"),
                    &ms.hotkey_label,
                    lv,
                    keys,
                    clips,
                );
                MessageBoxW(
                    hwnd,
                    wz(&text).as_ptr(),
                    wz(t(ms.lang, Msg::MenuAbout)).as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            CMD_GITHUB => crate::update::open_github(),
            CMD_RESET => {
                let answer = MessageBoxW(
                    hwnd,
                    wz(t(ms.lang, Msg::ResetConfirm)).as_ptr(),
                    wz(t(ms.lang, Msg::ResetTitle)).as_ptr(),
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
            CMD_UPDATE => {
                with_app(|a| {
                    if let Some(v) = a.pet.update_available().map(str::to_string) {
                        update::begin_install(&v);
                        a.pet.notify_update_downloading();
                    }
                });
            }
            CMD_AUTOUPDATE => {
                with_app(|a| {
                    a.pet.st.auto_update = !a.pet.st.auto_update;
                    update::set_enabled(a.pet.st.auto_update);
                    a.pet.dirty = true;
                });
            }
            CMD_PANEL => {
                with_app(|a| a.pet.toggle_panel());
            }
            CMD_CAPTURE => {
                with_app(|a| {
                    a.pet.st.clip_capture = !a.pet.st.clip_capture;
                    a.pet.dirty = true;
                });
            }
            CMD_AUTOCLOSE => {
                with_app(|a| a.pet.toggle_panel_autoclose());
            }
            CMD_HOTKEY => {
                with_app(|a| {
                    // core advances + persists the spec; we re-register the OS
                    // hotkey (which may fall back on a clash) and show the label
                    // that actually stuck.
                    let spec = a.pet.cycle_hotkey();
                    let reg = unsafe {
                        UnregisterHotKey(hwnd, HOTKEY_ID);
                        register_panel_hotkey(hwnd, &spec)
                    };
                    let label = reg.label();
                    a.pet.set_panel_hint(label.clone());
                    a.hotkey_label = label;
                    // If this preset is OS-reserved, replace cycle_hotkey's
                    // chord toast with the explicit fallback explanation.
                    if let HotkeyReg::Fallback { wanted, used } = &reg {
                        a.pet.notify_hotkey_fallback(wanted, used);
                    }
                });
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
            CMD_LANG_EN => {
                with_app(|a| {
                    a.pet.st.set_lang(Lang::En);
                    a.pet.dirty = true;
                });
            }
            CMD_LANG_KO => {
                with_app(|a| {
                    a.pet.st.set_lang(Lang::Ko);
                    a.pet.dirty = true;
                });
            }
            c if (CMD_SIZE0..CMD_SIZE0 + SCALES.len()).contains(&c) => {
                with_app(|a| {
                    a.pet.set_scale_idx(c - CMD_SIZE0);
                });
            }
            c if (CMD_SOUND0..CMD_SOUND0 + 3).contains(&c) => {
                with_app(|a| {
                    a.pet.st.sound_mode = (c - CMD_SOUND0) as u8;
                    a.pet.dirty = true;
                });
            }
            c if (CMD_WINLEVEL0..CMD_WINLEVEL0 + 3).contains(&c) => {
                with_app(|a| {
                    a.pet.set_window_level((c - CMD_WINLEVEL0) as u8);
                    a.apply_window_level();
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
                // restart request must run outside the APP borrow: DestroyWindow
                // re-enters the wndproc and WM_DESTROY needs `with_app`
                if with_app(|a| a.tick()).unwrap_or(false) {
                    update::restart();
                    DestroyWindow(hwnd);
                }
            }
            0
        }
        WM_CLIPBOARDUPDATE => {
            // while capture is paused the clipboard is not even read
            let capture = with_app(|a| {
                if !a.pet.st.clip_capture {
                    a.suppress_clip = None; // don't let a stale marker linger
                    false
                } else {
                    true
                }
            })
            .unwrap_or(false);
            if capture {
                if let Some((text, formats)) = read_clipboard_rich(hwnd) {
                    with_app(|a| {
                        if a.suppress_clip.as_deref() == Some(text.as_str()) {
                            a.suppress_clip = None;
                            return;
                        }
                        let (source, badge) = clipboard_source();
                        a.pet.on_copy(text, source, badge, formats);
                    });
                }
            }
            0
        }
        WM_HOTKEY => {
            if wp as i32 == HOTKEY_ID {
                with_app(|a| {
                    // the hotkey always *shows* the panel (never toggles closed)
                    // and reveals the window — un-hidden, raised and focused —
                    // so an obscured (Normal) or hidden panel reappears
                    a.pet.open_panel();
                    a.reveal();
                });
            }
            0
        }
        WM_LBUTTONDOWN => {
            with_app(|a| {
                let (cx, cy) = a.client_xy(lp);
                // header strip / resize grip start a card drag (the grip
                // pokes slightly past the card edge, hence the extra check)
                if a.pet.panel_drag_start(cx, cy) {
                    a.panel_drag = true;
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    a.drag_cursor = pt;
                    SetCapture(hwnd);
                    return;
                }
                if a.pet.panel_hit(cx, cy) {
                    // panel interactions act on press; no dragging from there
                    if let Some(pick) = a.pet.panel_click(cx, cy) {
                        a.copy_back(pick);
                    }
                    return;
                }
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
                let (cx, cy) = a.client_xy(lp);
                a.pet.set_cursor(cx, cy);
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
                if a.panel_drag {
                    // card drag: feed screen-pixel deltas as panel units (1.0);
                    // the tick applies the new layout (resize + shift)
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    let (dx, dy) = (pt.x - a.drag_cursor.x, pt.y - a.drag_cursor.y);
                    if dx != 0 || dy != 0 {
                        a.pet.panel_drag_update(dx as f32, dy as f32);
                        a.drag_cursor = pt;
                    }
                } else if a.mouse_down && !a.pet.st.locked {
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    let dx = pt.x - a.drag_cursor.x;
                    let dy = pt.y - a.drag_cursor.y;
                    if a.drag_moved || dx.abs() > 3 || dy.abs() > 3 {
                        a.drag_moved = true;
                        if a.pet.panel_open() {
                            // cat-body drag with the panel open: slide the cat,
                            // keep the card fixed (incremental, re-base each step;
                            // the tick applies the window shift). Don't move the
                            // whole window — that would drag the card along.
                            a.pet.drag_pet(dx as f32, dy as f32);
                            a.drag_cursor = pt;
                        } else {
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
                }
            });
            0
        }
        WM_MOUSELEAVE => {
            with_app(|a| {
                a.pet.set_hover(false);
                a.pet.clear_cursor();
                a.hover_tracking = false;
            });
            0
        }
        WM_LBUTTONUP => {
            with_app(|a| {
                if a.panel_drag {
                    a.panel_drag = false;
                    a.pet.panel_drag_end();
                    ReleaseCapture();
                    return;
                }
                if a.mouse_down {
                    a.mouse_down = false;
                    ReleaseCapture();
                    if a.drag_moved {
                        // a panel-open cat drag lives in the card offset (already
                        // persisted by drag_pet); only a plain window move needs
                        // its new screen position saved
                        if !a.pet.panel_open() {
                            let (x, y) = a.window_pos();
                            a.pet.save_pos(x, y);
                        }
                    } else {
                        let (cx, cy) = a.client_xy(lp);
                        let (lx, ly) = a.pet.cat_point(cx, cy);
                        a.pet.click_bounce(lx, ly);
                    }
                }
            });
            0
        }
        WM_LBUTTONDBLCLK => {
            with_app(|a| {
                a.mouse_down = false;
                let (cx, cy) = a.client_xy(lp);
                // a fast second header/grip drag arrives as a double-click
                if a.pet.panel_drag_start(cx, cy) {
                    a.panel_drag = true;
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    a.drag_cursor = pt;
                    SetCapture(hwnd);
                    return;
                }
                if a.pet.panel_hit(cx, cy) {
                    // fast row clicks arrive as double-clicks; treat as click
                    if let Some(pick) = a.pet.panel_click(cx, cy) {
                        a.copy_back(pick);
                    }
                } else {
                    a.pet.pet();
                }
            });
            0
        }
        WM_MBUTTONUP => {
            with_app(|a| a.pet.toggle_panel());
            0
        }
        WM_MOUSEWHEEL => {
            with_app(|a| {
                if a.pet.panel_open() {
                    let delta = ((wp >> 16) & 0xFFFF) as i16 as i32;
                    if delta != 0 {
                        a.pet.panel_wheel(if delta > 0 { -1 } else { 1 });
                    }
                }
            });
            0
        }
        WM_CHAR => {
            with_app(|a| {
                if a.pet.panel_open() {
                    a.panel_char_utf16(wp as u16);
                }
            });
            0
        }
        WM_KEYDOWN => {
            let handled = with_app(|a| {
                if !a.pet.panel_open() {
                    return false;
                }
                let ctrl = GetKeyState(0x11) < 0; // VK_CONTROL
                let key = match wp as u16 {
                    0x26 => Some(NavKey::Up),       // VK_UP
                    0x28 => Some(NavKey::Down),     // VK_DOWN
                    0x21 => Some(NavKey::PageUp),   // VK_PRIOR
                    0x22 => Some(NavKey::PageDown), // VK_NEXT
                    0x24 => Some(NavKey::Home),     // VK_HOME
                    0x23 => Some(NavKey::End),      // VK_END
                    0x0D => Some(NavKey::Enter),    // VK_RETURN
                    0x2E => Some(NavKey::Delete),   // VK_DELETE
                    0x08 => Some(NavKey::Backspace),// VK_BACK
                    0x1B => Some(NavKey::Esc),      // VK_ESCAPE
                    0x09 => Some(NavKey::Tab),      // VK_TAB: source filter
                    0x50 if ctrl => Some(NavKey::Pin),  // Ctrl+P: pin clip
                    0x5A if ctrl => Some(NavKey::Undo), // Ctrl+Z: undo delete
                    // Ctrl+0..9: quick-copy the nth row from the top
                    k @ 0x30..=0x39 if ctrl => Some(NavKey::Quick((k - 0x30) as u8)),
                    k @ 0x60..=0x69 if ctrl => Some(NavKey::Quick((k - 0x60) as u8)), // numpad
                    _ => None,
                };
                if let Some(key) = key {
                    if let Some(pick) = a.pet.panel_nav(key) {
                        a.copy_back(pick);
                    }
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
            if handled {
                0
            } else {
                DefWindowProcW(hwnd, msg, wp, lp)
            }
        }
        WM_RBUTTONUP => {
            open_menu(hwnd);
            0
        }
        WM_TRAY => {
            let ev = (lp & 0xFFFF) as u32;
            match ev {
                WM_LBUTTONUP => {
                    // toggle hide/show through the window level so the tray and
                    // the menu's "Hide" agree; un-hide restores the prior level
                    with_app(|a| {
                        if !a.pet.show_window() {
                            a.pet.set_window_level(2);
                        }
                        a.apply_window_level();
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
                UnregisterHotKey(hwnd, HOTKEY_ID);
                RemoveClipboardFormatListener(hwnd);
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

/// The work area (screen pixels, taskbar excluded) of the monitor nearest
/// `hwnd`. Used to keep a freshly opened panel on the same monitor as the pet.
fn monitor_work_rect(hwnd: HWND) -> Option<Rect> {
    unsafe {
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if mon.is_null() {
            return None;
        }
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return None;
        }
        let r = mi.rcWork;
        Some(Rect {
            x: r.left as f32,
            y: r.top as f32,
            w: (r.right - r.left) as f32,
            h: (r.bottom - r.top) as f32,
        })
    }
}

// ---- entry ------------------------------------------------------------------------------

pub fn run() {
    unsafe {
        // single instance
        CreateMutexW(null(), 0, wz("Local\\ClipCat-Singleton").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        SetProcessDpiAwarenessContext(-4 as _); // PER_MONITOR_AWARE_V2
        crate::sound::init();
        migrate_autostart();

        let hinst = GetModuleHandleW(null());
        let icon = make_icon();
        let class_name = wz("ClipCatWindow");

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

        // self-update plumbing: drop the `.old` exe a previous update left
        // behind, then start the daily release check (ADR-0009)
        update::cleanup_old_exe();
        update::set_enabled(st.auto_update);
        update::spawn_checker();

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
            wz("ClipCat").as_ptr(),
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

        // global panel hotkey (best-effort: a clash with another app falls
        // back to Ctrl+Shift+V; failing that, tray/middle-click still work)
        let reg = register_panel_hotkey(hwnd, &st.hotkey);
        let hotkey_label = reg.label();

        let mut pet = Pet::new(st);
        pet.set_panel_hint(hotkey_label.clone());
        // Tell the user when the configured chord was unavailable, so the
        // displayed fallback label is not a silent mismatch with state.json.
        if let HotkeyReg::Fallback { wanted, used } = &reg {
            pet.notify_hotkey_fallback(wanted, used);
        }

        let app = App {
            hwnd,
            mem_dc,
            dib,
            bits,
            w,
            h,
            pm: tiny_skia::Pixmap::new(w as u32, h as u32).unwrap(),
            pet,
            suppress_clip: None,
            mouse_down: false,
            drag_moved: false,
            panel_drag: false,
            drag_cursor: POINT { x: 0, y: 0 },
            drag_win: (0, 0),
            hover_tracking: false,
            visible: true,
            pending_surrogate: None,
            kbd_hook: SetWindowsHookExW(WH_KEYBOARD_LL, Some(kbd_hook), null_mut(), 0),
            mouse_hook: SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), null_mut(), 0),
            icon,
            taskbar_created: RegisterWindowMessageW(wz("TaskbarCreated").as_ptr()),
            hotkey_label,
            paste_target: null_mut(),
        };

        APP.with(|cell| *cell.borrow_mut() = Some(app));

        AddClipboardFormatListener(hwnd);

        tray_add(hwnd, icon);
        with_app(|a| a.update_tray_tip());
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        with_app(|a| {
            a.pet.render(&mut a.pm);
            a.blit();
        }); // first paint
        with_app(|a| a.apply_window_level()); // enforce a persisted level/hide
        SetTimer(hwnd, TIMER_ID, TICK_MS, None);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
