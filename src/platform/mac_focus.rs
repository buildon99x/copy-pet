//! macOS frontmost-app capture + re-activation for the clipboard flyout's
//! paste-back (Win+V parity).
//!
//! When the panel hotkey opens the flyout, the flyout window takes keyboard
//! focus (for the search box), so the source app is no longer frontmost by the
//! time the user picks a clip. To paste back into the original caret we capture
//! the source app before stealing focus, then re-activate it just before the
//! synthesized Cmd+V — the macOS analogue of the Windows-native backend's
//! `GetForegroundWindow` / `SetForegroundWindow` dance (see
//! [`super::mac_caret`] for the caret-position half of the same flow).
//!
//! Privacy (golden rule #1): this captures only a **process id** — never a
//! window title, app name, bundle id or any content.
//!
//! All `unsafe` here is AppKit (`NSWorkspace` / `NSRunningApplication`)
//! Objective-C messaging. No object is retained across calls: every `Id` is an
//! autoreleased AppKit instance used immediately and then dropped.

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

type Id = *mut Object;

/// Process id of the frontmost application, or `None`.
///
/// `[[NSWorkspace sharedWorkspace] frontmostApplication]` is the app currently
/// owning the menu bar / keyboard focus — read at hotkey time, before our
/// flyout activates, this is the app the user was typing in.
pub fn frontmost_app_pid() -> Option<i32> {
    unsafe {
        let ws: Id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if ws.is_null() {
            return None;
        }
        let app: Id = msg_send![ws, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let pid: i32 = msg_send![app, processIdentifier];
        // -1 is NSRunningApplication's "no pid"; guard against 0 too.
        if pid > 0 {
            Some(pid)
        } else {
            None
        }
    }
}

/// Re-activates the app with `pid` so a subsequent synthesized paste lands
/// there. Returns whether a running app with that pid was found.
///
/// Uses `activateWithOptions:` with `NSApplicationActivateIgnoringOtherApps`
/// so the switch happens even though *we* are the active app at call time. The
/// activation is asynchronous — the caller must give it a moment to land before
/// synthesizing the keystroke (there is no `AttachThreadInput` equivalent to
/// make it synchronous, as the Windows backend uses).
pub fn activate_app(pid: i32) -> bool {
    // NSApplicationActivationOptions: NSApplicationActivateIgnoringOtherApps.
    const ACTIVATE_IGNORING_OTHER_APPS: u64 = 1 << 1;
    unsafe {
        let app: Id = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if app.is_null() {
            return false;
        }
        let _: bool = msg_send![app, activateWithOptions: ACTIVATE_IGNORING_OTHER_APPS];
        true
    }
}
