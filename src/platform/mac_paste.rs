//! Output-only paste: capture the target before showing the flyout, restore it,
//! and wait for activation on the main run loop before posting a clean Cmd+V.
use super::mac_util::Id;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use objc::{class, msg_send, sel, sel_impl};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" { fn AXIsProcessTrusted() -> bool; }

pub fn trusted() -> bool {
    // SAFETY: side-effect-free system permission query; no input is observed.
    unsafe { AXIsProcessTrusted() }
}

pub fn frontmost_pid() -> Option<i32> {
    // SAFETY: called on the UI thread; objects live within this autorelease pool.
    unsafe {
        let pool: Id = msg_send![class!(NSAutoreleasePool), new];
        let workspace: Id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: Id = msg_send![workspace, frontmostApplication];
        let pid: i32 = if app.is_null() { 0 } else { msg_send![app, processIdentifier] };
        let _: () = msg_send![pool, drain];
        (pid > 0).then_some(pid)
    }
}

pub fn activate(pid: i32) -> bool {
    // SAFETY: main-thread AppKit messaging; no other app's contents are read.
    unsafe {
        let pool: Id = msg_send![class!(NSAutoreleasePool), new];
        let app: Id = msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
        let ok: bool = !app.is_null() && msg_send![app, activateWithOptions: 2usize];
        let _: () = msg_send![pool, drain];
        ok
    }
}

fn command_v(down: bool) -> Result<CGEvent, ()> {
    let source = CGEventSource::new(CGEventSourceStateID::Private)?;
    let event = CGEvent::new_keyboard_event(source, 9, down)?; // ANSI V
    // rdev creates V events without Command flags; modifier-only events do
    // not make that a reliable Cmd+V. Explicit flags also exclude held Shift.

    event.set_flags(CGEventFlags::CGEventFlagCommand);
    Ok(event)
}

pub fn paste() {
    for down in [true, false] {
        if let Ok(event) = command_v(down) { event.post(CGEventTapLocation::HID); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paste_events_carry_command_without_inheriting_hotkey_modifiers() {
        for down in [true, false] {
            assert_eq!(command_v(down).unwrap().get_flags(), CGEventFlags::CGEventFlagCommand);
        }
    }
}
