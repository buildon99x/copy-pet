//! OS clipboard privacy flags and application identity. Called at observation time.
use windows_sys::Win32::{Foundation::CloseHandle, System::{DataExchange::*, Memory::*, Threading::*}, UI::WindowsAndMessaging::*};

pub unsafe fn owner_path() -> Option<String> {
    let mut src = GetClipboardOwner();
    if src.is_null() {
        src = GetForegroundWindow();
    }
    if src.is_null() {
        return None;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(src, &mut pid);
    if pid == 0 {
        return None;
    }
    let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if proc.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(proc, 0, buf.as_mut_ptr(), &mut len);
    CloseHandle(proc);
    if ok == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    Some(path)
}

pub fn app_name(path: &str) -> Option<String> {
    std::path::Path::new(path).file_stem()?.to_str().map(str::to_owned)
}

/// Caller owns the clipboard lock. A malformed opt-out flag is treated as private.
pub unsafe fn marked_open() -> bool {
    static FORMATS: std::sync::OnceLock<[u32; 2]> = std::sync::OnceLock::new();
    let [exclude, history] = *FORMATS.get_or_init(|| {
        ["ExcludeClipboardContentFromMonitorProcessing", "CanIncludeInClipboardHistory"].map(|name| {
            let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            RegisterClipboardFormatW(wide.as_ptr())
        })
    });
    if IsClipboardFormatAvailable(exclude) != 0 { return true; }
    if IsClipboardFormatAvailable(history) == 0 { return false; }
    let handle = GetClipboardData(history);
    if handle.is_null() || GlobalSize(handle) < 4 { return true; }
    let data = GlobalLock(handle) as *const u32;
    if data.is_null() { return true; }
    let private = data.read_unaligned() == 0;
    GlobalUnlock(handle);
    private
}

#[cfg(feature = "portable")]
pub fn generation() -> isize {
    unsafe { GetClipboardSequenceNumber() as isize }
}

#[cfg(feature = "portable")]
pub fn observe(expected: isize) -> Option<(String, Option<String>, Option<crate::clipboard::RichFormats>)> {
    // SAFETY: the lock spans identity, privacy flags and text. The OS owns the
    // data handle and it is unlocked before closing the clipboard.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 { return None; }
        let result = (|| {
            if generation() != expected || marked_open() { return None; }
            let source = owner_path().and_then(|p| app_name(&p));
            let handle = GetClipboardData(13);
            if handle.is_null() { return None; }
            let size = GlobalSize(handle);
            if size > 2 * crate::clipboard::MAX_TEXT + 2 { return None; }
            let ptr = GlobalLock(handle) as *const u16;
            if ptr.is_null() { return None; }
            let slice = std::slice::from_raw_parts(ptr, size / 2);
            let len = slice.iter().position(|v| *v == 0).unwrap_or(slice.len());
            let text = String::from_utf16_lossy(&slice[..len]);
            GlobalUnlock(handle);
            Some((text, source, None))
        })();
        CloseClipboard();
        result
    }
}
