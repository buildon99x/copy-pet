//! Portable hotkey bridge. Receives only registered shortcut notifications,
//! never a global input stream. The owner unregisters and joins on drop.
#[cfg(not(target_os = "macos"))]
use crate::hotkey::Hotkey;
#[cfg(not(target_os = "macos"))]
use winit::event_loop::EventLoopProxy;

#[derive(Debug)]
pub enum HotkeyEvent {
    OpenPanel,
}

#[cfg(target_os = "macos")]
pub use super::mac_hotkey::RegisteredHotkey;

#[cfg(not(target_os = "macos"))]
pub struct RegisteredHotkey {
    tx: std::sync::mpsc::Sender<Command>,
    thread: Option<std::thread::JoinHandle<()>>,
}
#[cfg(not(target_os = "macos"))]
enum Command {
    Bind(Hotkey, std::sync::mpsc::SyncSender<bool>),
    Stop,
}
#[cfg(not(target_os = "macos"))]
impl RegisteredHotkey {
    pub fn new(proxy: EventLoopProxy<HotkeyEvent>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let mut native = native::Native::new();
            loop {
                match rx.recv_timeout(std::time::Duration::from_millis(20)) {
                    Ok(Command::Bind(hk, reply)) => {
                        let result = native.as_mut().is_some_and(|n| n.bind(hk));
                        let _ = reply.send(result);
                    }
                    Ok(Command::Stop) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
                if native.as_mut().is_some_and(|n| n.poll()) {
                    let _ = proxy.send_event(HotkeyEvent::OpenPanel);
                }
            }
        });
        Self {
            tx,
            thread: Some(thread),
        }
    }
    pub fn bind(&mut self, hk: Hotkey) -> bool {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.tx.send(Command::Bind(hk, tx)).is_ok() && rx.recv().unwrap_or(false)
    }
}
#[cfg(not(target_os = "macos"))]
impl Drop for RegisteredHotkey {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows_sys::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*};
    pub struct Native;
    impl Native {
        pub fn new() -> Option<Self> {
            Some(Self)
        }
        pub fn bind(&mut self, hk: Hotkey) -> bool {
            // SAFETY: all calls are confined to this thread's message queue.
            unsafe {
                UnregisterHotKey(std::ptr::null_mut(), 1);
                let mods = MOD_NOREPEAT
                    | if hk.ctrl { MOD_CONTROL } else { 0 }
                    | if hk.shift { MOD_SHIFT } else { 0 }
                    | if hk.alt { MOD_ALT } else { 0 }
                    | if hk.win { MOD_WIN } else { 0 };
                RegisterHotKey(std::ptr::null_mut(), 1, mods, hk.key as u32) != 0
            }
        }
        pub fn poll(&mut self) -> bool {
            let mut fired = false;
            // SAFETY: MSG is valid and this thread owns its queue.
            unsafe {
                let mut msg = std::mem::zeroed();
                while PeekMessageW(
                    &mut msg,
                    std::ptr::null_mut(),
                    WM_HOTKEY,
                    WM_HOTKEY,
                    PM_REMOVE,
                ) != 0
                {
                    fired = true;
                }
            }
            fired
        }
    }
    impl Drop for Native {
        fn drop(&mut self) {
            // SAFETY: registration and teardown share the owning message-loop thread.
            unsafe {
                UnregisterHotKey(std::ptr::null_mut(), 1);
            }
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod native {
    use super::*;
    use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
    type Display = c_void;
    #[repr(C)]
    struct ErrorEvent {
        kind: c_int,
        display: *mut Display,
        resource: c_ulong,
        serial: c_ulong,
        code: u8,
        request: u8,
        minor: u8,
    }
    #[repr(C)]
    struct KeyEvent {
        kind: c_int,
        serial: c_ulong,
        send: c_int,
        display: *mut Display,
        window: c_ulong,
        root: c_ulong,
        subwindow: c_ulong,
        time: c_ulong,
        x: c_int,
        y: c_int,
        x_root: c_int,
        y_root: c_int,
        state: c_uint,
        keycode: c_uint,
        same_screen: c_int,
    }
    #[repr(C)]
    struct ModifierMap {
        max: c_int,
        codes: *mut u8,
    }
    type ErrorHandler = unsafe extern "C" fn(*mut Display, *mut ErrorEvent) -> c_int;
    #[link(name = "X11")]
    extern "C" {
        fn XOpenDisplay(name: *const c_char) -> *mut Display;
        fn XCloseDisplay(d: *mut Display) -> c_int;
        fn XDefaultRootWindow(d: *mut Display) -> c_ulong;
        fn XKeysymToKeycode(d: *mut Display, sym: c_ulong) -> u8;
        fn XGrabKey(
            d: *mut Display,
            key: c_int,
            mods: c_uint,
            root: c_ulong,
            owner: c_int,
            pointer: c_int,
            keyboard: c_int,
        ) -> c_int;
        fn XUngrabKey(d: *mut Display, key: c_int, mods: c_uint, root: c_ulong) -> c_int;
        fn XSync(d: *mut Display, discard: c_int) -> c_int;
        fn XPending(d: *mut Display) -> c_int;
        fn XNextEvent(d: *mut Display, event: *mut c_long_event) -> c_int;
        fn XSetErrorHandler(handler: Option<ErrorHandler>) -> Option<ErrorHandler>;
        fn XGetModifierMapping(d: *mut Display) -> *mut ModifierMap;
        fn XFreeModifiermap(map: *mut ModifierMap) -> c_int;
    }
    // XEvent is a union of 24 longs on Xlib platforms. Only its type is read.
    #[allow(non_camel_case_types)]
    type c_long_event = [std::ffi::c_long; 24];
    static OUR_DISPLAY: AtomicPtr<Display> = AtomicPtr::new(std::ptr::null_mut());
    static FAILED: AtomicBool = AtomicBool::new(false);
    static PREVIOUS: std::sync::Mutex<Option<ErrorHandler>> = std::sync::Mutex::new(None);
    unsafe extern "C" fn error(d: *mut Display, event: *mut ErrorEvent) -> c_int {
        // Xlib's error handler is process-wide. Catch errors only on our private
        // connection and delegate every other connection to its original handler.
        if d == OUR_DISPLAY.load(Ordering::SeqCst) {
            FAILED.store(true, Ordering::SeqCst);
            0
        } else {
            let handler = *PREVIOUS.lock().unwrap_or_else(|e| e.into_inner());
            handler.map_or(0, |handler| handler(d, event))
        }
    }
    pub struct Native {
        display: *mut Display,
        root: c_ulong,
        key: u8,
        modifiers: Vec<u32>,
    }
    impl Native {
        pub fn new() -> Option<Self> {
            // XWayland grabs cannot provide desktop-global shortcuts on Wayland.
            if std::env::var_os("WAYLAND_DISPLAY").is_some()
                || std::env::var("XDG_SESSION_TYPE")
                    .is_ok_and(|s| s.eq_ignore_ascii_case("wayland"))
            {
                return None;
            }
            // SAFETY: private display connection used exclusively on this worker.
            unsafe {
                let display = XOpenDisplay(std::ptr::null());
                if display.is_null() {
                    return None;
                }
                OUR_DISPLAY.store(display, Ordering::SeqCst);
                *PREVIOUS.lock().unwrap_or_else(|e| e.into_inner()) = XSetErrorHandler(Some(error));
                Some(Self {
                    display,
                    root: XDefaultRootWindow(display),
                    key: 0,
                    modifiers: Vec::new(),
                })
            }
        }
        fn clear(&mut self) {
            // SAFETY: this worker owns the live display and every stored grab.
            unsafe {
                for mods in self.modifiers.drain(..) {
                    XUngrabKey(self.display, self.key.into(), mods, self.root);
                }
                XSync(self.display, 0);
                self.key = 0;
            }
        }
        pub fn bind(&mut self, hk: Hotkey) -> bool {
            // SAFETY: Xlib buffers and modifier map stay valid until their matching free.
            unsafe {
                self.clear();
                FAILED.store(false, Ordering::SeqCst);
                self.key = XKeysymToKeycode(self.display, hk.key.to_ascii_lowercase() as c_ulong);
                if self.key == 0 {
                    return false;
                }
                let base = u32::from(hk.shift)
                    | (u32::from(hk.ctrl) << 2)
                    | (u32::from(hk.alt) << 3)
                    | (u32::from(hk.win) << 6);
                let mut locks = 2; // CapsLock (LockMask)
                let map = XGetModifierMapping(self.display);
                let num = XKeysymToKeycode(self.display, 0xff7f); // Num_Lock
                if !map.is_null() {
                    for modifier in 0..8 {
                        for index in 0..(*map).max {
                            if num != 0
                                && *(*map).codes.add((modifier * (*map).max + index) as usize)
                                    == num
                            {
                                locks |= 1 << modifier;
                            }
                        }
                    }
                    XFreeModifiermap(map);
                }
                for mask in 0..256 {
                    if mask & !locks == 0 {
                        let mods = base | mask;
                        if !self.modifiers.contains(&mods) {
                            XGrabKey(self.display, self.key.into(), mods, self.root, 0, 1, 1);
                            self.modifiers.push(mods);
                        }
                    }
                }
                XSync(self.display, 0); // deliver BadAccess before claiming success
                if FAILED.load(Ordering::SeqCst) {
                    self.clear();
                    false
                } else {
                    true
                }
            }
        }
        pub fn poll(&mut self) -> bool {
            let mut fired = false;
            // SAFETY: XEvent has 24-long storage; Xlib fills it before typed access.
            unsafe {
                while XPending(self.display) > 0 {
                    let mut event = [0; 24];
                    XNextEvent(self.display, &mut event);
                    // XEvent.type is int, not long (works on either endian).
                    let key = &*event.as_ptr().cast::<KeyEvent>();
                    if key.kind == 2
                        && key.keycode == u32::from(self.key)
                        && self.modifiers.contains(&(key.state & 0xff))
                    {
                        fired = true;
                    }
                }
            }
            fired
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        #[ignore = "requires an X11 display; run explicitly against Xvfb"]
        fn x11_conflict_rebind_and_cleanup() {
            let mut native = Native::new().expect("X11 display");
            let hk = Hotkey::from_spec("ctrl+alt+k");
            assert!(native.bind(hk));
            unsafe {
                let other = XOpenDisplay(std::ptr::null());
                assert!(!other.is_null());
                let key = XKeysymToKeycode(other, 'k' as c_ulong);
                // Release our binding, then reserve the chord in another client.
                native.clear();
                XGrabKey(other, key.into(), 4 | 8, XDefaultRootWindow(other), 0, 1, 1);
                XSync(other, 0);
                assert!(
                    !native.bind(hk),
                    "BadAccess must return false, never exit process"
                );
                assert!(native.modifiers.is_empty(), "partial lock grabs cleaned up");
                assert!(
                    native.bind(Hotkey::from_spec("ctrl+alt+j")),
                    "rebind after conflict"
                );
                XCloseDisplay(other);
            }
            drop(native);
            let mut again = Native::new().unwrap();
            assert!(
                again.bind(Hotkey::from_spec("ctrl+alt+j")),
                "drop released chord"
            );
        }
    }
    impl Drop for Native {
        fn drop(&mut self) {
            // SAFETY: no display use remains after this owning worker exits.
            unsafe {
                self.clear();
                XCloseDisplay(self.display);
                let previous = PREVIOUS.lock().unwrap_or_else(|e| e.into_inner()).take();
                XSetErrorHandler(previous);
                OUR_DISPLAY.store(std::ptr::null_mut(), Ordering::SeqCst);
            }
        }
    }
}
