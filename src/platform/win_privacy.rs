//! Windows clipboard privacy and write receipts. Both backends match the owner
//! and finalized sequence under the read lock before publishing capture feedback.
use windows_sys::Win32::{Foundation::{CloseHandle, GlobalFree, HWND}, System::{DataExchange::*, Memory::*, Threading::*}, UI::WindowsAndMessaging::*};

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
pub(super) fn observe<P>(expected: isize, suppression: &std::sync::Mutex<Option<ClipboardIdentity>>,
    permit: impl FnOnce(Option<&str>) -> Option<P>) -> Result<Option<(String, Option<String>, Option<crate::clipboard::RichFormats>, P)>, ()> {
    // SAFETY: the lock spans identity, privacy flags and text. The OS owns the
    // data handle and it is unlocked before closing the clipboard.
    let mut marker = suppression.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 { return Err(()); }
        let skip = generation() != expected || ClipboardIdentity::consume_open(&mut marker);
        drop(marker); // Reading a delayed format can wait on its owner.
        let result = (|| {
            if skip || marked_open() { return None; }
            let source = owner_path().and_then(|p| app_name(&p));
            let permit = permit(source.as_deref())?;
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
            Some((text, source, None, permit))
        })();
        CloseClipboard();
        Ok(result)
    }
}

#[cfg(all(test, feature = "portable"))]
mod tests {
    use super::*;

    #[test]
    fn privacy_locked_clipboard_requests_retry() {
        // Take only the lock, never read or change the user's clipboard.
        unsafe {
            // A NULL owner associates the lock with the calling task. Model
            // window contention with an invisible system-class message window.
            let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
            let hwnd = CreateWindowExW(0, class.as_ptr(), std::ptr::null(), 0,
                0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(),
                std::ptr::null_mut(), std::ptr::null());
            assert!(!hwnd.is_null());
            let locked = (0..20).any(|_| {
                if OpenClipboard(hwnd) != 0 { return true; }
                std::thread::sleep(std::time::Duration::from_millis(10));
                false
            });
            if !locked { DestroyWindow(hwnd); panic!("could not acquire test clipboard lock"); }
            let holder = GetOpenClipboardWindow();
            let busy = std::thread::spawn(|| observe(generation(), &std::sync::Mutex::new(None), |_| Some(())).is_err()).join();
            CloseClipboard();
            DestroyWindow(hwnd);
            assert_eq!(holder, hwnd);
            assert!(busy.unwrap());
        }
    }
}

/// The HWND is only compared as an opaque identity. The portable clipboard
/// watcher can check it from another thread without accessing the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ClipboardIdentity {
    owner: usize,
    generation: u32,
}

impl ClipboardIdentity {
    /// SAFETY: the caller holds the clipboard open across both observations.
    unsafe fn current_open() -> Self {
        Self { owner: GetClipboardOwner() as usize, generation: GetClipboardSequenceNumber() }
    }

    /// Multiple posted notifications can all refer to the same current clipboard.
    #[cfg(not(feature = "portable"))]
    pub(super) unsafe fn skip_notification(observed: &mut Option<Self>, marker: &mut Option<Self>) -> bool {
        let current = Self::current_open();
        if current.generation != 0 && *observed == Some(current) { return true; }
        *observed = Some(current);
        Self::consume_open(marker)
    }

    /// Only a successful lock can resolve a pending receipt. A later owner or
    /// generation (even one publishing identical bytes) remains a real copy.
    pub(super) unsafe fn consume_open(marker: &mut Option<Self>) -> bool {
        marker.take().is_some_and(|expected| expected.generation != 0 && expected == Self::current_open())
    }
}

pub(super) struct WriteReceipt {
    identity: ClipboardIdentity,
    complete: bool,
}

/// All Windows writers end their write lock here. CloseClipboard finalizes
/// synthesized formats, so an earlier generation cannot identify the write.
pub(super) unsafe fn close_write(hwnd: HWND, completeness: Option<bool>) -> Option<WriteReceipt> {
    if CloseClipboard() == 0 { return None; }
    let complete = completeness?;
    let identity = ClipboardIdentity { owner: hwnd as usize, generation: GetClipboardSequenceNumber() };
    Some(WriteReceipt { identity, complete })
}

impl WriteReceipt {
    /// Arm capture suppression immediately. Only a second, read-only lock
    /// can verify the finalized owner and generation for paste dispatch.
    pub(super) fn acknowledge(
        self, marker: &mut Option<ClipboardIdentity>, require_complete: bool,
    ) -> Option<u32> {
        *marker = Some(self.identity);
        // SAFETY: each successful OpenClipboard is closed on this thread.
        unsafe {
            for attempt in 0..5 {
                if attempt != 0 { std::thread::sleep(std::time::Duration::from_millis(5)); }
                if OpenClipboard(std::ptr::null_mut()) == 0 { continue; }
                let current = ClipboardIdentity::current_open();
                let closed = CloseClipboard() != 0;
                if current != self.identity {
                    *marker = None;
                    return None;
                }
                return (closed && current.generation != 0 && current.generation == GetClipboardSequenceNumber()
                    && (!require_complete || self.complete)).then_some(current.generation);
            }
        }
        // Another reader may still be holding our successful write. Keep
        // the receipt until capture can compare it under its own read lock.
        None
    }
}

/// Writes one format's bytes to an already-open, already-emptied clipboard.
/// SAFETY: the GlobalAlloc'd buffer is handed to the clipboard on success (it
/// then owns it); on failure we free it ourselves.
pub(super) unsafe fn write_format(fmt: u32, bytes: &[u8]) -> bool {
    let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
    if h.is_null() {
        return false;
    }
    let p = GlobalLock(h) as *mut u8;
    if p.is_null() {
        GlobalFree(h);
        return false;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
    GlobalUnlock(h);
    if SetClipboardData(fmt, h).is_null() {
        GlobalFree(h);
        false
    } else {
        true
    }
}

/// Emit every captured rich format while the caller holds the clipboard lock.
pub(super) unsafe fn write_rich_formats(formats: &crate::clipboard::RichFormats) -> bool {
    fn atom(name: &str) -> u32 {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: wide is a terminated, live UTF-16 string.
        unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
    }
    let mut ok = true;
    if let Some(html) = &formats.html {
        let mut data = html.as_bytes().to_vec();
        data.push(0);
        ok &= write_format(atom("HTML Format"), &data);
    }
    if let Some(rtf) = &formats.rtf_b64 {
        let Some(mut data) = crate::clipboard::b64_decode(rtf) else {
            return false;
        };
        data.push(0);
        ok &= write_format(atom("Rich Text Format"), &data);
    }
    ok
}

/// Write CF_UNICODETEXT and every requested rich format under one lock.
/// SAFETY: hwnd is the live ClipCat clipboard owner window.
pub(super) unsafe fn write_text(
    hwnd: HWND, text: &str, formats: Option<&crate::clipboard::RichFormats>,
) -> Option<WriteReceipt> {
    write_text_when(hwnd, text, formats, || true)
}

/// The read-only preview can expire during a write-lock retry. Authorize the
/// selection after locking and before touching the destination clipboard.
pub(super) unsafe fn write_text_when(
    hwnd: HWND, text: &str, formats: Option<&crate::clipboard::RichFormats>, allowed: impl FnOnce() -> bool,
) -> Option<WriteReceipt> {
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let bytes = std::slice::from_raw_parts(wide.as_ptr().cast(), wide.len() * 2);
    for attempt in 0..5 {
        if attempt != 0 { std::thread::sleep(std::time::Duration::from_millis(10)); }
        if OpenClipboard(hwnd) == 0 { continue; }
        if !allowed() { CloseClipboard(); return None; }
        let text_written = EmptyClipboard() != 0 && write_format(13, bytes);
        let completeness = text_written.then(|| formats.is_none_or(|formats| write_rich_formats(formats)));
        return close_write(hwnd, completeness);
    }
    None
}

#[cfg(feature = "portable")]
pub(super) fn write_window_text(
    window: &winit::window::Window, text: &str, formats: Option<&crate::clipboard::RichFormats>,
) -> Option<WriteReceipt> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else { return None; };
    // SAFETY: window owns this non-null HWND for the duration of the write.
    unsafe { write_text(handle.hwnd.get() as _, text, formats) }
}
