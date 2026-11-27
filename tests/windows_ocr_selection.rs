//! Exercise the actual native panel, modal control, clipboard write and capture
//! listener in a dedicated synthetic profile. Text and clipboard bytes are
//! read only from the test's own editors or its child process's known output.
#![cfg(all(windows, not(feature = "portable")))]
use clipcat::clip_image::ClipImage;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command},
    ptr::null_mut,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::{DataExchange::*, Memory::*};
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*};

struct Fixture {
    child: Child,
    dir: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn wait<T>(label: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {label}");
        thread::sleep(Duration::from_millis(25));
    }
}

unsafe extern "system" fn find_main(hwnd: HWND, lp: LPARAM) -> BOOL {
    let (pid, found) = &mut *(lp as *mut (u32, HWND));
    let mut window_pid = 0;
    GetWindowThreadProcessId(hwnd, &mut window_pid);
    if window_pid == *pid
        && IsWindowVisible(hwnd) != 0
        && GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE != 0
    {
        *found = hwnd;
        return 0;
    }
    1
}

unsafe fn send(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> usize {
    let mut result = 0;
    assert_ne!(
        SendMessageTimeoutW(
            hwnd,
            message,
            wp,
            lp,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            3000,
            &mut result
        ),
        0,
        "child window did not respond to {message:#x}"
    );
    result
}

unsafe fn clipboard_is(owner: HWND, text: &str) -> bool {
    if GetClipboardOwner() != owner || OpenClipboard(null_mut()) == 0 {
        return false;
    }
    let matches = (|| {
        if GetClipboardOwner() != owner {
            return false;
        }
        let handle = GetClipboardData(13);
        if handle.is_null() {
            return false;
        }
        let expected: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        if GlobalSize(handle) < expected.len() * 2 {
            return false;
        }
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() {
            return false;
        }
        let same = std::slice::from_raw_parts(ptr, expected.len()) == expected;
        GlobalUnlock(handle);
        same
    })();
    CloseClipboard();
    matches
}

unsafe fn wait_for_clipboard(stage: &str, owner: HWND, text: &str) {
    let until = Instant::now() + Duration::from_secs(10);
    while !clipboard_is(owner, text) {
        assert!(Instant::now() < until, "{stage}: expected owner={owner:?}, current={:?}, sequence={}, locked_by={:?}",
            GetClipboardOwner(), GetClipboardSequenceNumber(), GetOpenClipboardWindow());
        thread::sleep(Duration::from_millis(15));
    }
}

unsafe fn click(pt: POINT, button: [MOUSE_EVENT_FLAGS; 2]) {
    assert_ne!(SetCursorPos(pt.x, pt.y), 0);
    let mut clicks: [INPUT; 2] = std::mem::zeroed();
    for (click, flags) in clicks.iter_mut().zip(button) {
        click.r#type = INPUT_MOUSE;
        click.Anonymous.mi.dwFlags = flags;
    }
    assert_eq!(
        SendInput(2, clicks.as_ptr(), std::mem::size_of::<INPUT>() as i32),
        2
    );
}

// The caller has checked the foreground dialog and focused EDIT. Send input
// through the same queue as the focus click, in physical event order.
unsafe fn send_keys(events: &[(u16, KEYBD_EVENT_FLAGS)]) {
    let inputs: Vec<INPUT> = events.iter().map(|&(key, flags)| {
        let mut input: INPUT = std::mem::zeroed();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = key;
        input.Anonymous.ki.dwFlags = flags;
        input
    }).collect();
    assert_eq!(SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32), inputs.len() as u32);
}

unsafe fn select_first_line_range(dialog: HWND, edit: HWND, start: u16, end: u16) {
    let mut gui: GUITHREADINFO = std::mem::zeroed();
    gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    assert_ne!(GetGUIThreadInfo(GetWindowThreadProcessId(dialog, null_mut()), &mut gui), 0);
    assert_eq!(GetForegroundWindow(), dialog);
    assert_eq!(gui.hwndFocus, edit);
    let tap = |key| [(key, 0), (key, KEYEVENTF_KEYUP)];
    let mut events = vec![(VK_CONTROL, 0)];
    events.extend(tap(VK_HOME));
    events.push((VK_CONTROL, KEYEVENTF_KEYUP));
    for _ in 0..start { events.extend(tap(VK_RIGHT)); }
    events.push((VK_SHIFT, 0));
    for _ in start..end { events.extend(tap(VK_RIGHT)); }
    events.push((VK_SHIFT, KEYEVENTF_KEYUP));
    send_keys(&events);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        // EM_GETSEL packs these short offsets when both pointers are NULL.
        let range = send(edit, 0x00b0, 0, 0);
        let actual = (range as u16, (range >> 16) as u16);
        if actual == (start, end) { break; }
        assert!(Instant::now() < deadline, "preview selection expected {start}..{end}, observed {actual:?}, foreground={:?}", GetForegroundWindow());
        thread::sleep(Duration::from_millis(15));
    }
}

fn preview_sample() -> String {
    let mut sample = String::from("Part 12345 receipt");
    for i in 1..=24 {
        sample.push_str(&format!("\nSynthetic line {i:02} · 한글 문서"));
    }
    sample
}

/// Print just the fixture child's owned dialog (including its standard EDIT).
/// The destination DIB is private memory. No desktop pixels are copied.
unsafe fn save_preview_image(dialog: HWND, directory: &std::path::Path) {
    use windows_sys::Win32::Graphics::Gdi::*;
    #[link(name = "user32")]
    extern "system" {
        fn PrintWindow(window: HWND, dc: HDC, flags: u32) -> BOOL;
    }
    let mut rect: RECT = std::mem::zeroed();
    assert_ne!(GetWindowRect(dialog, &mut rect), 0);
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let mut pixels = tiny_skia::Pixmap::new(width as u32, height as u32).unwrap();
    let dc = CreateCompatibleDC(null_mut());
    assert!(!dc.is_null());
    let mut info: BITMAPINFO = std::mem::zeroed();
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    let mut bits = null_mut();
    let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    assert!(!bitmap.is_null() && !bits.is_null());
    let previous = SelectObject(dc, bitmap);
    let printed = PrintWindow(dialog, dc, 0);
    GdiFlush();
    let bytes = std::slice::from_raw_parts(bits.cast::<u8>(), pixels.data().len());
    for (pixel, source) in pixels
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(bytes.as_chunks::<4>().0)
    {
        pixel.copy_from_slice(&[source[2], source[1], source[0], 255]);
    }
    SelectObject(dc, previous);
    DeleteObject(bitmap);
    DeleteDC(dc);
    assert_ne!(printed, 0);
    fs::create_dir_all(directory).unwrap();
    pixels
        .save_png(directory.join("native-ocr-preview.png"))
        .unwrap();
}

unsafe fn menu_command_point(owner: HWND, command: u32, mut pump: impl FnMut()) -> POINT {
    let thread = GetWindowThreadProcessId(owner, null_mut());
    unsafe extern "system" fn find_menu(hwnd: HWND, output: LPARAM) -> BOOL {
        let mut class = [0u16; 16];
        let n = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        if IsWindowVisible(hwnd) != 0 && String::from_utf16_lossy(&class[..n as usize]) == "#32768"
        {
            *(output as *mut HWND) = hwnd;
            return 0;
        }
        1
    }
    let root = GetAncestor(owner, GA_ROOT);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        pump();
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        let gui = GetGUIThreadInfo(thread, &mut info) != 0;
        let mut popup: HWND = null_mut();
        EnumThreadWindows(thread, Some(find_menu), &mut popup as *mut HWND as LPARAM);
        let mut menu = 0;
        let mut hit = null_mut();
        if gui && (info.hwndMenuOwner == owner || info.hwndMenuOwner == root) && !popup.is_null()
            && SendMessageTimeoutW(popup, MN_GETHMENU, 0, 0, SMTO_ABORTIFHUNG, 100, &mut menu) != 0
        {
            let menu = menu as HMENU;
            if let Some(index) = (0..GetMenuItemCount(menu)).find(|&i| GetMenuItemID(menu, i) == command) {
                let mut rect: RECT = std::mem::zeroed();
                if GetMenuItemRect(null_mut(), menu, index as u32, &mut rect) != 0 {
                    let point = POINT { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 };
                    hit = WindowFromPoint(point);
                    if hit == popup { return point; }
                }
            }
        }
        assert!(Instant::now() < deadline,
            "command {command} in menu of {owner:?}: root={root:?}, foreground={:?}, focus={:?}, menu owner={:?}, popup={popup:?}, menu={menu:#x}, hit={hit:?}",
            GetForegroundWindow(), info.hwndFocus, info.hwndMenuOwner);
        thread::sleep(Duration::from_millis(15));
    }
}

fn hosted_desktop() -> bool {
    std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true")
        && std::env::var("RUNNER_ENVIRONMENT").as_deref() == Ok("github-hosted")
}

// On hosted Windows the runner's Terminal can occupy the editor's click
// point even after the editor enters the topmost band. Minimize only that
// obstruction on the disposable VM, then restore its original placement.
#[derive(Default)]
struct RunnerTerminal(Option<(HWND, u32, WINDOWPLACEMENT)>);

impl RunnerTerminal {
    unsafe fn clear_obstruction(&mut self, hit: HWND) {
        if self.0.is_some() || !hosted_desktop() || hit.is_null() { return; }
        let window = GetAncestor(hit, GA_ROOT);
        let mut class = [0u16; 64];
        let len = GetClassNameW(window, class.as_mut_ptr(), class.len() as i32);
        if String::from_utf16_lossy(&class[..len as usize]) != "CASCADIA_HOSTING_WINDOW_CLASS" { return; }
        let mut placement: WINDOWPLACEMENT = std::mem::zeroed();
        placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        let mut pid = 0;
        assert_ne!(GetWindowThreadProcessId(window, &mut pid), 0);
        assert_ne!(GetWindowPlacement(window, &mut placement), 0);
        assert_ne!(ShowWindowAsync(window, SW_MINIMIZE), 0);
        self.0 = Some((window, pid, placement));
    }
}

impl Drop for RunnerTerminal {
    fn drop(&mut self) {
        if let Some((window, pid, placement)) = self.0.take() {
            unsafe {
                let mut current_pid = 0;
                if GetWindowThreadProcessId(window, &mut current_pid) != 0 && current_pid == pid {
                    SetWindowPlacement(window, &placement);
                }
            }
        }
    }
}

// Both editors belong to the fixture process. The first open seeds the
// previous target; the cat's real context-menu command must replace it with B.
unsafe fn queue_menu_roundtrip(main: HWND) {
    unsafe extern "system" fn editor_frame(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_SETFOCUS {
            let edit = GetDlgItem(hwnd, 1);
            if !edit.is_null() {
                SetFocus(edit);
                return 0;
            }
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
    let module = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
    let class: Vec<u16> = "ClipCatTestQueueEditor".encode_utf16().chain(Some(0)).collect();
    let mut wc: WNDCLASSW = std::mem::zeroed();
    wc.hInstance = module;
    wc.lpfnWndProc = Some(editor_frame);
    wc.lpszClassName = class.as_ptr();
    assert_ne!(RegisterClassW(&wc), 0);
    let editor = |x| {
        let frame = CreateWindowExW(
            0, class.as_ptr(), std::ptr::null(), WS_OVERLAPPEDWINDOW,
            x, 60, 200, 100, null_mut(), null_mut(), module, std::ptr::null(),
        );
        assert!(!frame.is_null());
        let edit_class: Vec<u16> = "EDIT".encode_utf16().chain(Some(0)).collect();
        let edit = CreateWindowExW(
            0, edit_class.as_ptr(), std::ptr::null(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_MULTILINE as u32,
            0, 0, 180, 50, frame, 1usize as HMENU, module, std::ptr::null(),
        );
        assert!(!edit.is_null());
        // The test's next click supplies activation after both controls exist.
        ShowWindow(frame, SW_SHOWNOACTIVATE);
        (frame, edit)
    };
    let (frame_a, edit_a) = editor(30);
    let (frame_b, edit_b) = editor(270);
    let clicked_edit = std::cell::Cell::new(null_mut());
    let pump = || {
        let mut msg = std::mem::zeroed();
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            if msg.message == WM_LBUTTONUP && (msg.hwnd == edit_a || msg.hwnd == edit_b) {
                clicked_edit.set(msg.hwnd);
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    };
    let mut runner_terminal = RunnerTerminal::default();
    let mut focus = |label: &str, frame: HWND, edit: HWND| {
        // A visible frame shown without activation can still be behind other
        // runner windows. Raise just this fixture frame for its physical click.
        let placement = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        assert_ne!(SetWindowPos(frame, HWND_TOPMOST, 0, 0, 0, 0, placement | SWP_SHOWWINDOW), 0);
        let deadline = Instant::now() + Duration::from_secs(10);
        let pt = loop {
            pump();
            let mut client: RECT = std::mem::zeroed();
            assert_ne!(GetClientRect(edit, &mut client), 0);
            let mut pt = POINT { x: client.right / 2, y: client.bottom / 2 };
            assert_ne!(windows_sys::Win32::Graphics::Gdi::ClientToScreen(edit, &mut pt), 0);
            let hit = WindowFromPoint(pt);
            if hit == edit { break pt; }
            runner_terminal.clear_obstruction(hit);
            if Instant::now() >= deadline {
                #[link(name = "dwmapi")]
                extern "system" {
                    fn DwmGetWindowAttribute(hwnd: HWND, attribute: u32, value: *mut u32, size: u32) -> i32;
                }
                let mut cloaked = 0;
                let cloak_status = DwmGetWindowAttribute(frame, 14 /* DWMWA_CLOAKED */, &mut cloaked, 4);
                let mut pid = 0;
                GetWindowThreadProcessId(hit, &mut pid);
                let mut class = [0u16; 256];
                let len = GetClassNameW(hit, class.as_mut_ptr(), class.len() as i32);
                panic!("editor {label} before click: frame={frame:?} visible={} minimized={} exstyle={:#x} cloaked={:?} previous_z={:?}, EDIT={edit:?} visible={} enabled={} dpi={}; point=({}, {}), hit={hit:?}, pid={pid}, class={}, root={:?}, foreground={:?}, runner terminal minimize requested={}",
                    IsWindowVisible(frame), IsIconic(frame), GetWindowLongW(frame, GWL_EXSTYLE),
                    (cloak_status >= 0).then_some(cloaked), GetWindow(frame, GW_HWNDPREV),
                    IsWindowVisible(edit), IsWindowEnabled(edit), GetDpiForWindow(edit), pt.x, pt.y,
                    String::from_utf16_lossy(&class[..len as usize]), GetAncestor(hit, GA_ROOT),
                    GetForegroundWindow(), runner_terminal.0.is_some());
            }
            thread::sleep(Duration::from_millis(15));
        };
        click(pt, [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP]);
        let deadline = Instant::now() + Duration::from_secs(10);
        let thread_id = GetWindowThreadProcessId(frame, null_mut());
        loop {
            pump();
            let mut gui: GUITHREADINFO = std::mem::zeroed();
            gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            let ok = GetGUIThreadInfo(thread_id, &mut gui) != 0;
            let foreground = GetForegroundWindow();
            if clicked_edit.get() == edit && ok && foreground == frame && gui.hwndActive == frame && gui.hwndFocus == edit {
                break;
            }
            assert!(Instant::now() < deadline,
                "editor {label} click: expected frame={frame:?}, EDIT={edit:?}; GUI info={ok}, foreground={foreground:?}, active={:?}, focus={:?}, hit={:?}, mouse-up={:?}",
                gui.hwndActive, gui.hwndFocus, WindowFromPoint(pt), clicked_edit.get());
            thread::sleep(Duration::from_millis(15));
        }
        assert_ne!(SetWindowPos(frame, HWND_NOTOPMOST, 0, 0, 0, 0, placement), 0);
    };
    let visible_panel = |open| {
        wait("embedded panel activation style", || {
            pump();
            ((GetWindowLongW(main, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE == 0) == open)
                .then_some(())
        });
    };
    visible_panel(false);
    focus("A", frame_a, edit_a);
    PostMessageW(main, WM_MBUTTONUP, 0, 0);
    visible_panel(true);
    send(main, WM_KEYDOWN, 0x24, 0); // Home selects the synthetic text row
    send(main, WM_KEYDOWN, 0x72, 0); // F3 adds it to the ordered queue
    PostMessageW(main, WM_MBUTTONUP, 0, 0);
    visible_panel(false);
    focus("B", frame_b, edit_b);
    let mut pet_bounds: RECT = std::mem::zeroed();
    assert_ne!(GetWindowRect(main, &mut pet_bounds), 0);
    let point = (pet_bounds.top..pet_bounds.bottom)
        .step_by(5)
        .find_map(|y| {
            (pet_bounds.left..pet_bounds.right)
                .step_by(5)
                .map(|x| POINT { x, y })
                .find(|pt| WindowFromPoint(*pt) == main)
        })
        .expect("visible hit-testable part of the synthetic pet");
    click(point, [MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP]);
    let thread = GetWindowThreadProcessId(main, null_mut());
    wait("native popup owned by the fixture pet", || {
        pump();
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        (GetGUIThreadInfo(thread, &mut info) != 0
            && info.hwndMenuOwner == main
            && GetForegroundWindow() == main)
            .then_some(())
    });
    let point = menu_command_point(main, 56, pump);
    click(point, [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP]);
    visible_panel(true);
    // Post so the editor thread keeps dispatching during checked focus restore.
    PostMessageW(main, WM_KEYDOWN, VK_RETURN as usize, 0);
    wait("queue text in editor B", || {
        pump();
        let mut text = [0u16; 32];
        let len = GetWindowTextW(edit_b, text.as_mut_ptr(), 32);
        (String::from_utf16_lossy(&text[..len as usize]) == "external fixture").then_some(())
    });
    assert_eq!(GetWindowTextLengthW(edit_a), 0);
    assert_eq!(GetForegroundWindow(), frame_b);
    assert_eq!(GetFocus(), edit_b);
    DestroyWindow(frame_a);
    DestroyWindow(frame_b);
    UnregisterClassW(class.as_ptr(), module);
}

fn sample_image(timestamp: u64, pinned: bool) -> serde_json::Value {
    let image = ClipImage::from_pixmap(&tiny_skia::Pixmap::new(2, 2).unwrap()).unwrap();
    json!({"id":1,"source":"Synthetic","text":"","ts":timestamp,"pinned":pinned,"image":image,"ocr_text":preview_sample()})
}

fn launch(clips: Vec<serde_json::Value>) -> Fixture {
    let dir = std::env::temp_dir().join(format!(
        "clipcat-preview-selection-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("clips.json"), json!({"clips":clips}).to_string()).unwrap();
    fs::write(
        dir.join("state.json"),
        json!({"onboarded":true,"auto_update":false,"retention_days":1,"clip_capture":true,"lang":"en"}).to_string(),
    )
    .unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_clipcat"))
        .env("CLIPCAT_CONFIG_DIR", &dir)
        .spawn()
        .unwrap();
    Fixture { child, dir }
}

unsafe fn main_window(fixture: &mut Fixture) -> HWND {
    wait("isolated ClipCat window", || {
        assert!(
            fixture.child.try_wait().unwrap().is_none(),
            "test app exited before window creation; check for another ClipCat in this session"
        );
        let mut target = (fixture.child.id(), null_mut() as HWND);
        EnumWindows(Some(find_main), &mut target as *mut (u32, HWND) as LPARAM);
        (!target.1.is_null()).then_some(target.1)
    })
}

unsafe fn open_preview(fixture: &mut Fixture) -> (HWND, HWND, HWND) {
    let main = main_window(fixture);
    PostMessageW(main, WM_MBUTTONUP, 0, 0);
    wait("embedded panel activation before F9", || {
        (GetWindowLongW(main, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE == 0).then_some(())
    });
    PostMessageW(main, WM_KEYDOWN, 0x78, 0); // F9 opens the actual preview
    let (dialog, edit) = wait("owned preview EDIT", || {
        let dialog = GetWindow(main, GW_ENABLEDPOPUP);
        if dialog.is_null() || dialog == main || GetWindow(dialog, GW_OWNER) != main {
            return None;
        }
        let edit = GetDlgItem(dialog, 100);
        if edit.is_null() || IsWindowVisible(dialog) == 0 || IsWindowVisible(edit) == 0 {
            return None;
        }
        let mut rect: RECT = std::mem::zeroed();
        (GetClientRect(edit, &mut rect) != 0 && rect.right > 100 && rect.bottom > 30)
            .then_some((dialog, edit))
    });
    (main, dialog, edit)
}

// One normal GUI occupies a Windows logon session. Each scenario gets an
// independent profile and child, and a failed scenario cannot skip the others.
fn desktop_session() -> Option<std::sync::MutexGuard<'static, ()>> {
    static DESKTOP: std::sync::Mutex<()> = std::sync::Mutex::new(());
    (hosted_desktop() || std::env::var("CLIPCAT_TEST_DESKTOP").as_deref() == Ok("1")).then(|| {
        let guard = DESKTOP.lock().unwrap_or_else(|e| e.into_inner());
        // The app uses per-monitor v2. The driver must hit-test and click in
        // that same coordinate space, including on a scaled runner display.
        unsafe { assert!(!SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_null()); }
        guard
    })
}

#[test]
fn copying_preview_selection_keeps_image_history_and_suppresses_copy_feedback() {
    let Some(_desktop) = desktop_session() else {
        return;
    };
    let mut fixture = launch(vec![sample_image(1, true)]);
    unsafe {
        let (main, dialog, edit) = open_preview(&mut fixture);
        // A real user can focus this app-owned selectable control by clicking.
        // Posted button notifications alone do not confer foreground rights.
        let mut point = POINT { x: 15, y: 15 };
        assert_ne!(
            windows_sys::Win32::Graphics::Gdi::ClientToScreen(edit, &mut point),
            0
        );
        assert_eq!(
            WindowFromPoint(point),
            edit,
            "preview EDIT must be visible before click"
        );
        click(point, [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP]);
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let mut gui: GUITHREADINFO = std::mem::zeroed();
            gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            let ok = GetGUIThreadInfo(GetWindowThreadProcessId(dialog, null_mut()), &mut gui) != 0;
            let foreground = GetForegroundWindow();
            if ok && foreground == dialog && gui.hwndFocus == edit {
                break;
            }
            assert!(Instant::now() < until,
                "after click: GUI info={ok}, fg={foreground:?}, active={:?}, focus={:?}, dialog={dialog:?}, edit={edit:?}, owner={main:?}, flags={:#x}",
                gui.hwndActive, gui.hwndFocus, gui.flags);
            thread::sleep(Duration::from_millis(15));
        }
        select_first_line_range(dialog, edit, 5, 10);
        if let Some(dir) = std::env::var_os("CLIPCAT_OCR_EVIDENCE") {
            save_preview_image(dialog, std::path::Path::new(&dir));
        }
        // The Apps key opens the selected text's context menu. Clicking Copy
        // invokes WM_COPY from within the app's ordinary menu dispatch.
        send_keys(&[(VK_APPS, 0), (VK_APPS, KEYEVENTF_KEYUP)]);
        let point = menu_command_point(edit, 1, || {});
        click(point, [MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP]);
        wait_for_clipboard("preview Copy menu WM_COPY selection", main, "12345");
        select_first_line_range(dialog, edit, 11, 18);
        send_keys(&[(VK_CONTROL, 0), (0x43, 0), (0x43, KEYEVENTF_KEYUP), (VK_CONTROL, KEYEVENTF_KEYUP)]);
        wait_for_clipboard("preview Ctrl+C selection", main, "receipt");
        // Allow the real asynchronous WM_CLIPBOARDUPDATE to be delivered before
        // shutting down. Receiving later messages in the modal loop is essential.
        PostMessageW(dialog, WM_CLOSE, 0, 0);
        wait("preview dismissal", || {
            (IsWindow(dialog) == 0).then_some(())
        });
        send(main, WM_NULL, 0, 0);
        // Publish a single independent Win32 change with an explicit owner.
        // It must reach the shipping listener after both self-written selections.
        let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
        let sample: Vec<u16> = "external fixture".encode_utf16().chain(Some(0)).collect();
        let external = CreateWindowExW(
            0, class.as_ptr(), std::ptr::null(), 0, 0, 0, 0, 0,
            HWND_MESSAGE, null_mut(), null_mut(), std::ptr::null(),
        );
        assert!(!external.is_null());
        let data = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(sample.as_slice()));
        assert!(!data.is_null());
        let dst = GlobalLock(data) as *mut u16;
        if dst.is_null() {
            GlobalFree(data);
            panic!("external clipboard fixture could not lock synthetic data");
        }
        std::ptr::copy_nonoverlapping(sample.as_ptr(), dst, sample.len());
        GlobalUnlock(data);
        wait("external fixture clipboard lock", || (OpenClipboard(external) != 0).then_some(()));
        let written = EmptyClipboard() != 0 && !SetClipboardData(13, data).is_null();
        let closed = CloseClipboard() != 0;
        if !written { GlobalFree(data); } // A successful SetClipboardData transfers ownership to Windows.
        assert!(written && closed, "external CF_UNICODETEXT write and close");
        wait_for_clipboard("external positive control", external, "external fixture");
        wait("capture of independent synthetic clipboard change", || {
            // ClipCat's session-query handler flushes history. Only the fixture
            // process receives this message; the test never ends the OS session.
            send(main, WM_QUERYENDSESSION, 0, 0);
            let bytes = fs::read(fixture.dir.join("clips.json")).ok()?;
            let data: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
            data["clips"]
                .as_array()?
                .iter()
                .any(|c| c["text"] == "external fixture")
                .then_some(())
        });
        DestroyWindow(external);
        PostMessageW(main, WM_CLOSE, 0, 0);
        assert!(wait("clean test app exit", || fixture.child.try_wait().unwrap()).success());
    }
    let stored: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.dir.join("clips.json")).unwrap()).unwrap();
    // The capture path starts fish feedback only after adding the captured clip.
    // A regressing native EDIT writer would insert the selected text as a new row.
    let rows = stored["clips"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|c| c["ocr_text"] == preview_sample()));
    assert!(rows.iter().any(|c| c["text"] == "external fixture"));
    assert!(!rows.iter().any(|c| c["text"] == "12345"));
}

#[test]
fn context_menu_queue_returns_to_the_new_external_editor() {
    let Some(_desktop) = desktop_session() else {
        return;
    };
    let clip = json!({"id":1,"source":"Synthetic", "text":"external fixture","pinned":false,"ts":clipcat::clipboard::now_ts()});
    let mut fixture = launch(vec![clip]);
    unsafe {
        let main = main_window(&mut fixture);
        queue_menu_roundtrip(main);
        PostMessageW(main, WM_CLOSE, 0, 0);
        assert!(wait("queue test app exit", || fixture.child.try_wait().unwrap()).success());
    }
}

#[test]
fn live_preview_closes_and_refuses_copy_at_the_retention_deadline() {
    let Some(_desktop) = desktop_session() else {
        return;
    };
    let now = clipcat::clipboard::now_ts();
    let timestamp = now - 86_400 + 6;
    let mut expiring = launch(vec![sample_image(timestamp, false)]);
    unsafe {
        let (main, dialog, edit) = open_preview(&mut expiring);
        send(edit, 0x00b1, 5, 10);
        let previous = GetClipboardSequenceNumber();
        wait("OCR preview expiry", || {
            if clipcat::clipboard::now_ts() <= timestamp + 86_400 {
                return None;
            }
            if IsWindow(edit) != 0 {
                SendMessageW(edit, WM_COPY, 0, 0);
            }
            (IsWindow(dialog) == 0).then_some(())
        });
        assert_eq!(
            GetClipboardSequenceNumber(),
            previous,
            "expired preview wrote to clipboard"
        );
        PostMessageW(main, WM_CLOSE, 0, 0);
        assert!(wait("expiring app exit", || expiring.child.try_wait().unwrap()).success());
    }
}
