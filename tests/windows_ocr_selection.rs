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

#[test]
fn copying_preview_selection_keeps_image_history_and_suppresses_copy_feedback() {
    if std::env::var_os("GITHUB_ACTIONS").is_none() && std::env::var_os("CLIPCAT_TEST_DESKTOP").is_none() {
        return;
    }
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
    let clip = json!({"id":1,"source":"Synthetic", "text":"", "ts":1,"pinned":true,
        "image":image,"ocr_text":"Part 12345 receipt"});
    fs::write(dir.join("clips.json"), json!({"clips":[clip]}).to_string()).unwrap();
    fs::write(
        dir.join("state.json"),
        json!({"onboarded":true,"auto_update":false,"clip_capture":true,"lang":"en"}).to_string(),
    )
    .unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_clipcat"))
        .env("CLIPCAT_CONFIG_DIR", &dir)
        .spawn()
        .unwrap();
    let mut fixture = Fixture { child, dir };
    unsafe {
        let main = wait("isolated ClipCat window", || {
            assert!(fixture.child.try_wait().unwrap().is_none(), "test app exited before window creation; check for another ClipCat in this session");
            let mut target = (fixture.child.id(), null_mut() as HWND);
            EnumWindows(Some(find_main), &mut target as *mut (u32, HWND) as LPARAM);
            (!target.1.is_null()).then_some(target.1)
        });
        PostMessageW(main, WM_MBUTTONUP, 0, 0);
        PostMessageW(main, WM_KEYDOWN, 0x78, 0); // F9 opens the actual preview
        let (dialog, edit) = wait("owned preview EDIT", || {
            let dialog = GetWindow(main, GW_ENABLEDPOPUP);
            if dialog.is_null() || dialog == main || GetWindow(dialog, GW_OWNER) != main {
                return None;
            }
            let edit = GetDlgItem(dialog, 100);
            (!edit.is_null()).then_some((dialog, edit))
        });
        send(edit, 0x00b1, 5, 10); // EM_SETSEL, select only 12345
        send(edit, WM_COPY, 0, 0); // same command as the read-only context menu
        assert!(
            clipboard_is(main, "12345"),
            "selection must go through ClipCat's plain-text writer"
        );
        send(edit, 0x00b1, 11, 18); // select "receipt" for the real Ctrl+C chord
        SetForegroundWindow(dialog);
        wait("foreground modal with focused synthetic EDIT", || {
            let mut gui: GUITHREADINFO = std::mem::zeroed();
            gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            (GetForegroundWindow() == dialog && GetGUIThreadInfo(GetWindowThreadProcessId(dialog, null_mut()), &mut gui) != 0
                && gui.hwndFocus == edit).then_some(())
        });
        let mut keys: [INPUT; 4] = std::mem::zeroed();
        for (input, (key, flags)) in keys.iter_mut().zip([(VK_CONTROL, 0), (0x43, 0), (0x43, KEYEVENTF_KEYUP), (VK_CONTROL, KEYEVENTF_KEYUP)]) {
            input.r#type = INPUT_KEYBOARD;
            input.Anonymous.ki.wVk = key;
            input.Anonymous.ki.dwFlags = flags;
        }
        assert_eq!(SendInput(4, keys.as_ptr(), std::mem::size_of::<INPUT>() as i32), 4);
        wait("Ctrl+C of selected receipt through the native writer", || clipboard_is(main, "receipt").then_some(()));
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
        PostMessageW(main, WM_CLOSE, 0, 0);
        assert!(wait("clean test app exit", || fixture.child.try_wait().unwrap()).success());
    }
    let stored: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.dir.join("clips.json")).unwrap()).unwrap();
    // The capture path starts fish feedback only after adding the captured clip.
    // A regressing native EDIT writer would insert the selected text as a new row.
    let rows = stored["clips"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|c| c["ocr_text"] == "Part 12345 receipt"));
    assert!(rows.iter().any(|c| c["text"] == "external fixture"));
    assert!(!rows.iter().any(|c| c["text"] == "12345"));
}
