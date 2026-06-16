//! macOS rich-format clipboard read/write via `NSPasteboard` (ADR-0014): the
//! macOS half of Win+V parity. On copy the portable watcher reads the original
//! HTML/RTF alongside the plain text; on paste `set_clipboard` re-emits them
//! (unless the user chose "paste as text", which writes plain text only).
//!
//! All `unsafe` here is Objective-C messaging, wrapped in an autorelease pool.
//! Only pasteboard data is touched — never a Text Input Source API — so this is
//! safe to call from the watcher thread (cf. LNR-0005, which is TIS-specific).

#![allow(unexpected_cfgs)] // objc 0.2's msg_send!/class! macros

use objc::runtime::{Object, BOOL};
use objc::{class, msg_send, sel, sel_impl};
use std::os::raw::c_char;

use crate::clipboard::{b64_decode, b64_encode, RichFormats};

type Id = *mut Object;

// NSPasteboard uniform-type identifiers.
const UTF8_TEXT: &str = "public.utf8-plain-text";
const HTML: &str = "public.html";
const RTF: &str = "public.rtf";

fn nsstring(s: &str) -> Id {
    let bytes = s.as_bytes();
    unsafe {
        msg_send![class!(NSString),
            stringWithBytes: bytes.as_ptr()
            length: bytes.len()
            encoding: 4usize] // NSUTF8StringEncoding
    }
}

unsafe fn nsstring_to_string(ns: Id) -> Option<String> {
    if ns.is_null() {
        return None;
    }
    let utf8: *const c_char = msg_send![ns, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

unsafe fn nsdata_to_vec(data: Id) -> Option<Vec<u8>> {
    if data.is_null() {
        return None;
    }
    let len: usize = msg_send![data, length];
    let ptr: *const u8 = msg_send![data, bytes];
    if len == 0 || ptr.is_null() {
        return None;
    }
    Some(std::slice::from_raw_parts(ptr, len).to_vec())
}

/// Reads the original rich formats (HTML/RTF) from the general pasteboard, or
/// `None` when the current item carries neither.
pub fn read_formats() -> Option<RichFormats> {
    unsafe {
        let pool: Id = msg_send![class!(NSAutoreleasePool), new];
        let pb: Id = msg_send![class!(NSPasteboard), generalPasteboard];
        let html: Id = msg_send![pb, stringForType: nsstring(HTML)];
        let html = nsstring_to_string(html);
        let rtf: Id = msg_send![pb, dataForType: nsstring(RTF)];
        let rtf_b64 = nsdata_to_vec(rtf).map(|b| b64_encode(&b));
        let _: () = msg_send![pool, drain];
        let f = RichFormats { html, rtf_b64 };
        if f.is_empty() {
            None
        } else {
            Some(f)
        }
    }
}

/// Replaces the pasteboard with `text` plus the given rich `formats` (when
/// present). `formats: None` writes plain text only.
pub fn write(text: &str, formats: Option<&RichFormats>) {
    unsafe {
        let pool: Id = msg_send![class!(NSAutoreleasePool), new];
        let pb: Id = msg_send![class!(NSPasteboard), generalPasteboard];
        let _: isize = msg_send![pb, clearContents];
        // plain text is always written, so the suppression marker (keyed on
        // text) stays valid for the watcher's self-copy skip.
        let _: BOOL = msg_send![pb, setString: nsstring(text) forType: nsstring(UTF8_TEXT)];
        if let Some(f) = formats {
            if let Some(html) = &f.html {
                let _: BOOL = msg_send![pb, setString: nsstring(html) forType: nsstring(HTML)];
            }
            if let Some(rtf_b64) = &f.rtf_b64 {
                if let Some(bytes) = b64_decode(rtf_b64) {
                    let data: Id =
                        msg_send![class!(NSData), dataWithBytes: bytes.as_ptr() length: bytes.len()];
                    let _: BOOL = msg_send![pb, setData: data forType: nsstring(RTF)];
                }
            }
        }
        let _: () = msg_send![pool, drain];
    }
}
