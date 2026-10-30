//! Carbon registered shortcuts on the UI/main run loop. No event tap or TIS.
use super::registered_hotkey::HotkeyEvent;
use crate::hotkey::Hotkey;
use std::ffi::c_void;
use winit::event_loop::EventLoopProxy;
#[repr(C)]
#[derive(Clone, Copy)]
struct HotkeyId {
    signature: u32,
    id: u32,
}
#[repr(C)]
struct EventType {
    class: u32,
    kind: u32,
}
type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        callback: Callback,
        count: u32,
        types: *const EventType,
        data: *mut c_void,
        handler: *mut *mut c_void,
    ) -> i32;
    fn RemoveEventHandler(handler: *mut c_void) -> i32;
    fn RegisterEventHotKey(
        key: u32,
        modifiers: u32,
        id: HotkeyId,
        target: *mut c_void,
        options: u32,
        hotkey: *mut *mut c_void,
    ) -> i32;
    fn UnregisterEventHotKey(hotkey: *mut c_void) -> i32;
}
unsafe extern "C" fn fired(_: *mut c_void, _: *mut c_void, data: *mut c_void) -> i32 {
    // SAFETY: boxed proxy outlives the handler; callbacks and drop run on UI thread.
    let proxy = &*(data as *const EventLoopProxy<HotkeyEvent>);
    let _ = proxy.send_event(HotkeyEvent::OpenPanel);
    0
}
pub struct RegisteredHotkey {
    handler: *mut c_void,
    hotkey: *mut c_void,
    _proxy: Box<EventLoopProxy<HotkeyEvent>>,
    // Prevent moving registration/teardown away from the main event loop.
    _main: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl RegisteredHotkey {
    pub fn new(proxy: EventLoopProxy<HotkeyEvent>) -> Self {
        let mut proxy = Box::new(proxy);
        let mut handler = std::ptr::null_mut();
        let event = EventType {
            class: u32::from_be_bytes(*b"keyb"),
            kind: 5,
        };
        // SAFETY: created by portable::run on the main thread; stable boxed userdata.
        unsafe {
            InstallEventHandler(
                GetApplicationEventTarget(),
                fired,
                1,
                &event,
                (&mut *proxy as *mut EventLoopProxy<HotkeyEvent>).cast(),
                &mut handler,
            );
        }
        Self {
            handler,
            hotkey: std::ptr::null_mut(),
            _proxy: proxy,
            _main: std::marker::PhantomData,
        }
    }
    pub fn bind(&mut self, hk: Hotkey) -> bool {
        // Physical ANSI keycodes, same layout-independent convention as the
        // previous shortcut. No text input source lookup or keyboard observation.
        let key = match hk.key {
            'A' => 0,
            'B' => 11,
            'C' => 8,
            'D' => 2,
            'E' => 14,
            'F' => 3,
            'G' => 5,
            'H' => 4,
            'I' => 34,
            'J' => 38,
            'K' => 40,
            'L' => 37,
            'M' => 46,
            'N' => 45,
            'O' => 31,
            'P' => 35,
            'Q' => 12,
            'R' => 15,
            'S' => 1,
            'T' => 17,
            'U' => 32,
            'V' => 9,
            'W' => 13,
            'X' => 7,
            'Y' => 16,
            'Z' => 6,
            '0' => 29,
            '1' => 18,
            '2' => 19,
            '3' => 20,
            '4' => 21,
            '5' => 23,
            '6' => 22,
            '7' => 26,
            '8' => 28,
            '9' => 25,
            _ => return false,
        };
        // SAFETY: the UI thread owns both the registration and application target.
        unsafe {
            if !self.hotkey.is_null() {
                UnregisterEventHotKey(self.hotkey);
                self.hotkey = std::ptr::null_mut();
            }
            if self.handler.is_null() {
                return false;
            }
            let modifiers = (u32::from(hk.win) << 8)
                | (u32::from(hk.shift) << 9)
                | (u32::from(hk.alt) << 11)
                | (u32::from(hk.ctrl) << 12);
            RegisterEventHotKey(
                key,
                modifiers,
                HotkeyId {
                    signature: u32::from_be_bytes(*b"ClCt"),
                    id: 1,
                },
                GetApplicationEventTarget(),
                0,
                &mut self.hotkey,
            ) == 0
        }
    }
}
impl Drop for RegisteredHotkey {
    fn drop(&mut self) {
        // SAFETY: main-thread teardown removes callbacks before boxed userdata drops.
        unsafe {
            if !self.hotkey.is_null() {
                UnregisterEventHotKey(self.hotkey);
            }
            if !self.handler.is_null() {
                RemoveEventHandler(self.handler);
            }
        }
    }
}
