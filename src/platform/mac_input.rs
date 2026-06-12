//! macOS global-input listener — a minimal CoreGraphics event tap.
//!
//! This replaces `rdev::listen` on macOS. rdev's macOS path calls the Text
//! Input Source Manager (`TSMGetInputSourceProperty`) on *every* key press to
//! fill `Event.name` (the typed character). Since macOS 15 those TIS APIs
//! assert they run on the **main dispatch queue** and hard-abort (SIGTRAP,
//! `dispatch_assert_queue`) when called from the tap's background thread —
//! crashing ClipCat on the first keystroke (Ctrl+C and any other). See
//! [LNR-0005](../../.context/kb/lnr/0005-macos-tis-eventtap-crash.md).
//!
//! ClipCat never uses the translated text, so this tap reads only the event
//! *kind* and the raw keycode and never touches TIS. That also tightens the
//! privacy boundary (golden rule #1 / ADR-0008): a keycode is mapped to the
//! `rdev::Key` the `ChordTracker` already understands and immediately
//! discarded — no key text is ever produced, stored or transmitted.
//!
//! Linux keeps using `rdev::listen` (X11 has no equivalent bug).
//!
//! The keycode table is the public-domain mapping from `rdev`'s
//! `macos/keycodes.rs` (MIT); kept here so the chord matches exactly what the
//! old rdev path matched.

#![allow(non_upper_case_globals)]
// CGEvent (a foreign_type! wrapper) crosses the FFI boundary by value, exactly
// as rdev does it — the pointer layout is what the system passes/expects.
#![allow(improper_ctypes_definitions)]
#![allow(improper_ctypes)]

use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGKeyCode, EventField,
};
use rdev::{Button, EventType, Key};
use std::os::raw::c_void;
use std::ptr;

// --- CoreGraphics / CoreFoundation FFI (the subset we need) ------------------
type CFMachPortRef = *const c_void;
type CFRunLoopSourceRef = *const c_void;
type CFRunLoopRef = *const c_void;
type CFRunLoopMode = *const c_void;
type CFAllocatorRef = *const c_void;
type CGEventTapProxy = *const c_void;
type CFIndex = isize;

const kCGHeadInsertEventTap: u32 = 0;
const kCGEventTapOptionListenOnly: u32 = 1;

/// The tap callback. `CGEvent` is passed/returned by value exactly as rdev
/// does it: the system owns the event, we read fields through a borrow and
/// hand the same pointer back (a ListenOnly tap never mutates it).
type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    etype: CGEventType,
    event: CGEvent,
    user_info: *mut c_void,
) -> CGEvent;

#[link(name = "Cocoa", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: CGEventTapLocation,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        tap: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFRunLoopMode);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CFRunLoopRun();
    static kCFRunLoopCommonModes: CFRunLoopMode;
}

const fn bit(t: CGEventType) -> u64 {
    1u64 << (t as u64)
}
/// Only the events the activity counters and the chord matcher care about —
/// keeps callback wake-ups (and CPU) to a minimum.
const EVENT_MASK: u64 = bit(CGEventType::KeyDown)
    | bit(CGEventType::KeyUp)
    | bit(CGEventType::FlagsChanged)
    | bit(CGEventType::ScrollWheel)
    | bit(CGEventType::LeftMouseDown)
    | bit(CGEventType::RightMouseDown)
    | bit(CGEventType::OtherMouseDown);

/// Per-listener state, held in a static because the C tap callback has no user
/// data we can thread through ergonomically. Touched only from the single tap
/// thread (the one that called [`listen`]).
struct Handler {
    callback: Box<dyn FnMut(EventType)>,
    /// Previous modifier bitmask — a FlagsChanged event is a press if more
    /// bits are now set, a release otherwise (mirrors rdev's heuristic).
    last_flags: CGEventFlags,
}

static mut HANDLER: Option<Handler> = None;
static mut TAP: CFMachPortRef = ptr::null();

unsafe extern "C" fn raw_callback(
    _proxy: CGEventTapProxy,
    etype: CGEventType,
    cg_event: CGEvent,
    _user_info: *mut c_void,
) -> CGEvent {
    // The OS disables the tap if our callback is ever too slow, or on certain
    // input transitions; re-enable it so we keep receiving events.
    if matches!(
        etype,
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
    ) {
        CGEventTapEnable(TAP, true);
        return cg_event;
    }
    let handler = &mut *ptr::addr_of_mut!(HANDLER);
    if let Some(h) = handler.as_mut() {
        if let Some(event) = convert(etype, &cg_event, &mut h.last_flags) {
            (h.callback)(event);
        }
    }
    cg_event
}

/// Maps a raw CG event to the `rdev::EventType` the rest of the portable
/// backend already speaks — without ever translating a keycode to text.
fn convert(etype: CGEventType, ev: &CGEvent, last_flags: &mut CGEventFlags) -> Option<EventType> {
    match etype {
        CGEventType::KeyDown => {
            let code = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as CGKeyCode;
            Some(EventType::KeyPress(key_from_code(code)))
        }
        CGEventType::KeyUp => {
            let code = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as CGKeyCode;
            Some(EventType::KeyRelease(key_from_code(code)))
        }
        CGEventType::FlagsChanged => {
            let code = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as CGKeyCode;
            let flags = ev.get_flags();
            let released = flags < *last_flags;
            *last_flags = flags;
            let key = key_from_code(code);
            Some(if released {
                EventType::KeyRelease(key)
            } else {
                EventType::KeyPress(key)
            })
        }
        CGEventType::ScrollWheel => {
            let delta_y =
                ev.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1);
            let delta_x =
                ev.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2);
            Some(EventType::Wheel { delta_x, delta_y })
        }
        CGEventType::LeftMouseDown => Some(EventType::ButtonPress(Button::Left)),
        CGEventType::RightMouseDown => Some(EventType::ButtonPress(Button::Right)),
        CGEventType::OtherMouseDown => Some(EventType::ButtonPress(Button::Middle)),
        _ => None,
    }
}

/// Installs the global event tap on the calling thread and runs its run loop
/// forever. Returns `Err` immediately if the tap cannot be created — which on
/// macOS means Accessibility permission has not been granted (the app stays
/// alive; the caller surfaces a hint).
pub fn listen<F>(callback: F) -> Result<(), ()>
where
    F: FnMut(EventType) + 'static,
{
    unsafe {
        HANDLER = Some(Handler {
            callback: Box::new(callback),
            last_flags: CGEventFlags::CGEventFlagNull,
        });
        let tap = CGEventTapCreate(
            CGEventTapLocation::HID,
            kCGHeadInsertEventTap,
            kCGEventTapOptionListenOnly,
            EVENT_MASK,
            raw_callback,
            ptr::null_mut(),
        );
        if tap.is_null() {
            HANDLER = None;
            return Err(());
        }
        TAP = tap;
        let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
        if source.is_null() {
            HANDLER = None;
            return Err(());
        }
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        CFRunLoopRun();
    }
    Ok(())
}

// --- keycode → rdev::Key (from rdev macos/keycodes.rs, MIT) ------------------
const ALT: CGKeyCode = 58;
const ALT_GR: CGKeyCode = 61;
const BACKSPACE: CGKeyCode = 51;
const CAPS_LOCK: CGKeyCode = 57;
const CONTROL_LEFT: CGKeyCode = 59;
const CONTROL_RIGHT: CGKeyCode = 62;
const DOWN_ARROW: CGKeyCode = 125;
const ESCAPE: CGKeyCode = 53;
const F1: CGKeyCode = 122;
const F10: CGKeyCode = 109;
const F11: CGKeyCode = 103;
const F12: CGKeyCode = 111;
const F2: CGKeyCode = 120;
const F3: CGKeyCode = 99;
const F4: CGKeyCode = 118;
const F5: CGKeyCode = 96;
const F6: CGKeyCode = 97;
const F7: CGKeyCode = 98;
const F8: CGKeyCode = 100;
const F9: CGKeyCode = 101;
const FUNCTION: CGKeyCode = 63;
const LEFT_ARROW: CGKeyCode = 123;
const META_LEFT: CGKeyCode = 55;
const META_RIGHT: CGKeyCode = 54;
const RETURN: CGKeyCode = 36;
const RIGHT_ARROW: CGKeyCode = 124;
const SHIFT_LEFT: CGKeyCode = 56;
const SHIFT_RIGHT: CGKeyCode = 60;
const SPACE: CGKeyCode = 49;
const TAB: CGKeyCode = 48;
const UP_ARROW: CGKeyCode = 126;
const BACK_QUOTE: CGKeyCode = 50;
const NUM1: CGKeyCode = 18;
const NUM2: CGKeyCode = 19;
const NUM3: CGKeyCode = 20;
const NUM4: CGKeyCode = 21;
const NUM5: CGKeyCode = 23;
const NUM6: CGKeyCode = 22;
const NUM7: CGKeyCode = 26;
const NUM8: CGKeyCode = 28;
const NUM9: CGKeyCode = 25;
const NUM0: CGKeyCode = 29;
const MINUS: CGKeyCode = 27;
const EQUAL: CGKeyCode = 24;
const KEY_Q: CGKeyCode = 12;
const KEY_W: CGKeyCode = 13;
const KEY_E: CGKeyCode = 14;
const KEY_R: CGKeyCode = 15;
const KEY_T: CGKeyCode = 17;
const KEY_Y: CGKeyCode = 16;
const KEY_U: CGKeyCode = 32;
const KEY_I: CGKeyCode = 34;
const KEY_O: CGKeyCode = 31;
const KEY_P: CGKeyCode = 35;
const LEFT_BRACKET: CGKeyCode = 33;
const RIGHT_BRACKET: CGKeyCode = 30;
const KEY_A: CGKeyCode = 0;
const KEY_S: CGKeyCode = 1;
const KEY_D: CGKeyCode = 2;
const KEY_F: CGKeyCode = 3;
const KEY_G: CGKeyCode = 5;
const KEY_H: CGKeyCode = 4;
const KEY_J: CGKeyCode = 38;
const KEY_K: CGKeyCode = 40;
const KEY_L: CGKeyCode = 37;
const SEMI_COLON: CGKeyCode = 41;
const QUOTE: CGKeyCode = 39;
const BACK_SLASH: CGKeyCode = 42;
const KEY_Z: CGKeyCode = 6;
const KEY_X: CGKeyCode = 7;
const KEY_C: CGKeyCode = 8;
const KEY_V: CGKeyCode = 9;
const KEY_B: CGKeyCode = 11;
const KEY_N: CGKeyCode = 45;
const KEY_M: CGKeyCode = 46;
const COMMA: CGKeyCode = 43;
const DOT: CGKeyCode = 47;
const SLASH: CGKeyCode = 44;

fn key_from_code(code: CGKeyCode) -> Key {
    match code {
        ALT => Key::Alt,
        ALT_GR => Key::AltGr,
        BACKSPACE => Key::Backspace,
        CAPS_LOCK => Key::CapsLock,
        CONTROL_LEFT => Key::ControlLeft,
        CONTROL_RIGHT => Key::ControlRight,
        DOWN_ARROW => Key::DownArrow,
        ESCAPE => Key::Escape,
        F1 => Key::F1,
        F10 => Key::F10,
        F11 => Key::F11,
        F12 => Key::F12,
        F2 => Key::F2,
        F3 => Key::F3,
        F4 => Key::F4,
        F5 => Key::F5,
        F6 => Key::F6,
        F7 => Key::F7,
        F8 => Key::F8,
        F9 => Key::F9,
        LEFT_ARROW => Key::LeftArrow,
        META_LEFT => Key::MetaLeft,
        META_RIGHT => Key::MetaRight,
        RETURN => Key::Return,
        RIGHT_ARROW => Key::RightArrow,
        SHIFT_LEFT => Key::ShiftLeft,
        SHIFT_RIGHT => Key::ShiftRight,
        SPACE => Key::Space,
        TAB => Key::Tab,
        UP_ARROW => Key::UpArrow,
        BACK_QUOTE => Key::BackQuote,
        NUM1 => Key::Num1,
        NUM2 => Key::Num2,
        NUM3 => Key::Num3,
        NUM4 => Key::Num4,
        NUM5 => Key::Num5,
        NUM6 => Key::Num6,
        NUM7 => Key::Num7,
        NUM8 => Key::Num8,
        NUM9 => Key::Num9,
        NUM0 => Key::Num0,
        MINUS => Key::Minus,
        EQUAL => Key::Equal,
        KEY_Q => Key::KeyQ,
        KEY_W => Key::KeyW,
        KEY_E => Key::KeyE,
        KEY_R => Key::KeyR,
        KEY_T => Key::KeyT,
        KEY_Y => Key::KeyY,
        KEY_U => Key::KeyU,
        KEY_I => Key::KeyI,
        KEY_O => Key::KeyO,
        KEY_P => Key::KeyP,
        LEFT_BRACKET => Key::LeftBracket,
        RIGHT_BRACKET => Key::RightBracket,
        KEY_A => Key::KeyA,
        KEY_S => Key::KeyS,
        KEY_D => Key::KeyD,
        KEY_F => Key::KeyF,
        KEY_G => Key::KeyG,
        KEY_H => Key::KeyH,
        KEY_J => Key::KeyJ,
        KEY_K => Key::KeyK,
        KEY_L => Key::KeyL,
        SEMI_COLON => Key::SemiColon,
        QUOTE => Key::Quote,
        BACK_SLASH => Key::BackSlash,
        KEY_Z => Key::KeyZ,
        KEY_X => Key::KeyX,
        KEY_C => Key::KeyC,
        KEY_V => Key::KeyV,
        KEY_B => Key::KeyB,
        KEY_N => Key::KeyN,
        KEY_M => Key::KeyM,
        COMMA => Key::Comma,
        DOT => Key::Dot,
        SLASH => Key::Slash,
        FUNCTION => Key::Function,
        code => Key::Unknown(code.into()),
    }
}
