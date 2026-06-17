//! Shared helpers for macOS Objective-C modules.

#![allow(unexpected_cfgs)] // objc 0.2's msg_send!/class! macros

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

pub(super) type Id = *mut Object;

/// Wraps a `&str` as an `NSString` (UTF-8). Caller is responsible for
/// memory: convenience temporaries are mopped up by `NSAutoreleasePool`.
pub(super) fn nsstring(s: &str) -> Id {
    let bytes = s.as_bytes();
    unsafe {
        msg_send![class!(NSString),
            stringWithBytes: bytes.as_ptr()
            length: bytes.len()
            encoding: 4usize] // NSUTF8StringEncoding
    }
}
