//! Exercise the actual native panel, modal control, clipboard write and capture
//! listener in a dedicated synthetic profile. Only the child process's windows
//! and a clipboard owned by its main window are inspected.
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

unsafe fn send(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) {
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

unsafe fn click(pt: POINT) {
    assert_ne!(SetCursorPos(pt.x, pt.y), 0);
    let mut clicks: [INPUT; 2] = std::mem::zeroed();
    for (click, flags) in clicks
        .iter_mut()
        .zip([MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP])
    {
        click.r#type = INPUT_MOUSE;
        click.Anonymous.mi.dwFlags = flags;
    }
    assert_eq!(
        SendInput(2, clicks.as_ptr(), std::mem::size_of::<INPUT>() as i32),
        2
    );
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

// Both editors belong to the fixture process. The first open seeds the
// previous target; the cat's real context-menu command must replace it with B.
unsafe fn queue_menu_roundtrip(main: HWND) {
    let editor = |x| {
        let class: Vec<u16> = "EDIT".encode_utf16().chain(Some(0)).collect();
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            [0u16].as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE | ES_MULTILINE as u32,
            x,
            60,
            200,
            100,
            null_mut(),
            null_mut(),
            null_mut(),
            std::ptr::null(),
        );
        assert!(!hwnd.is_null());
        hwnd
    };
    let a = editor(30);
    let b = editor(270);
    let pump = || {
        let mut msg = std::mem::zeroed();
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    };
    let focus = |editor| {
        // A real click gives this fixture the foreground entitlement and
        // focuses exactly its EDIT, independent of Windows foreground locks.
        let mut pt = POINT { x: 10, y: 10 };
        windows_sys::Win32::Graphics::Gdi::ClientToScreen(editor, &mut pt);
        assert_eq!(WindowFromPoint(pt), editor);
        click(pt);
        wait("synthetic editor focus", || {
            pump();
            (GetForegroundWindow() == editor && GetFocus() == editor).then_some(())
        });
    };
    let visible_panel = |open| {
        wait("embedded panel activation style", || {
            pump();
            ((GetWindowLongW(main, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE == 0) == open)
                .then_some(())
        });
    };
    // The preview left the embedded panel open. Close, then reopen from A.
    PostMessageW(main, WM_MBUTTONUP, 0, 0);
    visible_panel(false);
    focus(a);
    PostMessageW(main, WM_MBUTTONUP, 0, 0);
    visible_panel(true);
    send(main, WM_KEYDOWN, 0x24, 0); // Home selects the pinned image
    send(main, WM_KEYDOWN, 0x28, 0); // Down selects the external text row
    send(main, WM_KEYDOWN, 0x72, 0); // F3 adds it to the ordered queue
    PostMessageW(main, WM_MBUTTONUP, 0, 0);
    visible_panel(false);
    focus(b);
    PostMessageW(main, WM_RBUTTONUP, 0, 0);
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
    let point = wait("queue command in native menu", || {
        pump();
        let mut popup: HWND = null_mut();
        EnumThreadWindows(thread, Some(find_menu), &mut popup as *mut HWND as LPARAM);
        if popup.is_null() {
            return None;
        }
        let mut menu = 0;
        if SendMessageTimeoutW(popup, MN_GETHMENU, 0, 0, SMTO_ABORTIFHUNG, 100, &mut menu) == 0 {
            return None;
        }
        let menu = menu as HMENU;
        let index = (0..GetMenuItemCount(menu)).find(|&i| GetMenuItemID(menu, i) == 56)?;
        let mut rect: RECT = std::mem::zeroed();
        if GetMenuItemRect(null_mut(), menu, index as u32, &mut rect) == 0 {
            return None;
        }
        let point = POINT {
            x: (rect.left + rect.right) / 2,
            y: (rect.top + rect.bottom) / 2,
        };
        (WindowFromPoint(point) == popup).then_some(point)
    });
    click(point);
    visible_panel(true);
    // Post so the editor thread keeps dispatching during checked focus restore.
    PostMessageW(main, WM_KEYDOWN, VK_RETURN as usize, 0);
    wait("queue text in editor B", || {
        pump();
        let mut text = [0u16; 32];
        let len = GetWindowTextW(b, text.as_mut_ptr(), 32);
        (String::from_utf16_lossy(&text[..len as usize]) == "external fixture").then_some(())
    });
    assert_eq!(GetWindowTextLengthW(a), 0);
    assert_eq!(GetFocus(), b);
    DestroyWindow(a);
    DestroyWindow(b);
}

fn launch(timestamp: u64, pinned: bool) -> Fixture {
    let dir = std::env::temp_dir().join(format!(
        "clipcat-preview-selection-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    let image = ClipImage::from_pixmap(&tiny_skia::Pixmap::new(2, 2).unwrap()).unwrap();
    let clip = json!({"id":1,"source":"Synthetic", "text":"", "ts":timestamp,"pinned":pinned,
        "image":image,"ocr_text":preview_sample()});
    fs::write(dir.join("clips.json"), json!({"clips":[clip]}).to_string()).unwrap();
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

unsafe fn open_preview(fixture: &mut Fixture) -> (HWND, HWND, HWND) {
    let main = wait("isolated ClipCat window", || {
        assert!(
            fixture.child.try_wait().unwrap().is_none(),
            "test app exited before window creation; check for another ClipCat in this session"
        );
        let mut target = (fixture.child.id(), null_mut() as HWND);
        EnumWindows(Some(find_main), &mut target as *mut (u32, HWND) as LPARAM);
        (!target.1.is_null()).then_some(target.1)
    });
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

#[test]
fn copying_preview_selection_keeps_image_history_and_suppresses_copy_feedback() {
    if std::env::var_os("GITHUB_ACTIONS").is_none()
        && std::env::var_os("CLIPCAT_TEST_DESKTOP").is_none()
    {
        return;
    }
    let mut fixture = launch(1, true);
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
        click(point);
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
        send(edit, 0x00b1, 5, 10); // EM_SETSEL, select only 12345
        if let Some(dir) = std::env::var_os("CLIPCAT_OCR_EVIDENCE") {
            save_preview_image(dialog, std::path::Path::new(&dir));
        }
        send(edit, WM_COPY, 0, 0); // same command as the read-only context menu
        wait("one WM_COPY through ClipCat's plain-text writer", || {
            clipboard_is(main, "12345").then_some(())
        });
        send(edit, 0x00b1, 11, 18); // select "receipt" after the focus click
        let mut keys: [INPUT; 4] = std::mem::zeroed();
        for (input, (key, flags)) in keys.iter_mut().zip([
            (VK_CONTROL, 0),
            (0x43, 0),
            (0x43, KEYEVENTF_KEYUP),
            (VK_CONTROL, KEYEVENTF_KEYUP),
        ]) {
            input.r#type = INPUT_KEYBOARD;
            input.Anonymous.ki.wVk = key;
            input.Anonymous.ki.dwFlags = flags;
        }
        assert_eq!(
            SendInput(4, keys.as_ptr(), std::mem::size_of::<INPUT>() as i32),
            4
        );
        wait(
            "Ctrl+C of selected receipt through the native writer",
            || clipboard_is(main, "receipt").then_some(()),
        );
        // Allow the real asynchronous WM_CLIPBOARDUPDATE to be delivered before
        // shutting down. Receiving later messages in the modal loop is essential.
        PostMessageW(dialog, WM_CLOSE, 0, 0);
        wait("preview dismissal", || {
            (IsWindow(dialog) == 0).then_some(())
        });
        send(main, WM_NULL, 0, 0);
        // A normal, external EDIT writes through Windows itself. Confirm that the
        // live listener captures this positive control with capture enabled.
        let class: Vec<u16> = "EDIT".encode_utf16().chain(Some(0)).collect();
        let sample: Vec<u16> = "external fixture".encode_utf16().chain(Some(0)).collect();
        let external = CreateWindowExW(
            0,
            class.as_ptr(),
            sample.as_ptr(),
            WS_POPUP,
            0,
            0,
            100,
            30,
            null_mut(),
            null_mut(),
            null_mut(),
            std::ptr::null(),
        );
        assert!(!external.is_null());
        send(external, 0x00b1, 0, -1);
        send(external, WM_COPY, 0, 0);
        assert!(clipboard_is(external, "external fixture"));
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
        queue_menu_roundtrip(main);
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

    // Run sequentially on the same isolated desktop after the first child
    // exits. Cross the deadline while the real modal loop is open.
    let now = clipcat::clipboard::now_ts();
    let timestamp = now - 86_400 + 6;
    let mut expiring = launch(timestamp, false);
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
