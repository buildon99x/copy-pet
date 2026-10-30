//! macOS native modal dialogs (`NSAlert`) for the context menu's About box — the parity equivalent of the Windows
//! `MessageBoxW` calls. All `unsafe` is Objective-C messaging on the main
//! thread; `runModal` blocks until the user answers.

#![allow(unexpected_cfgs)] // objc 0.2's msg_send!/class! macros

use objc::{class, msg_send, sel, sel_impl};

use super::mac_util::{nsstring, Id};

unsafe fn make_alert(title: &str, body: &str) -> Id {
    let alert: Id = msg_send![class!(NSAlert), alloc];
    let alert: Id = msg_send![alert, init];
    let _: () = msg_send![alert, setMessageText: nsstring(title)];
    let _: () = msg_send![alert, setInformativeText: nsstring(body)];
    alert
}

/// Information dialog with a single OK button (the About box).
pub fn info(title: &str, body: &str) {
    unsafe {
        let pool: Id = msg_send![class!(NSAutoreleasePool), new];
        let alert = make_alert(title, body);
        let _: () = msg_send![alert, addButtonWithTitle: nsstring("OK")];
        let _: isize = msg_send![alert, runModal];
        let _: () = msg_send![alert, release];
        let _: () = msg_send![pool, drain];
    }
}
