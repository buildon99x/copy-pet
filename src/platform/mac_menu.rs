//! macOS right-click context menu — a native `NSMenu` popup.
//!
//! The portable backend's window is a small, borderless layer; a self-drawn
//! menu can't extend past it, so on macOS we pop a real `NSMenu` at the cursor
//! (it draws in its own window, like the Windows tray menu). The menu only
//! *labels* the actions — selecting one runs the same code path as the
//! corresponding keyboard shortcut, so there is no duplicated behavior here.
//!
//! All `unsafe` is Objective-C messaging. `popup` blocks in AppKit's nested
//! tracking run loop until the menu is dismissed; the chosen item's tag lands
//! in `SELECTED` via the one registered handler, which we read back after.
//! Objects we `alloc` are released; convenience-constructed temporaries are
//! mopped up by the autorelease pool around the popup.

#![allow(unexpected_cfgs)] // objc 0.2's msg_send!/class!/sel! macros

use core_graphics::geometry::CGPoint;
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel, BOOL, NO};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Once;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

type Id = *mut Object;

/// Tag of the last picked item (-1 = nothing picked). The menu is modal, so a
/// single slot is enough — no two popups overlap.
static SELECTED: AtomicI64 = AtomicI64::new(-1);

extern "C" fn menu_picked(_this: &Object, _cmd: Sel, item: Id) {
    let tag: isize = unsafe { msg_send![item, tag] };
    SELECTED.store(tag as i64, Ordering::SeqCst);
}

/// A shared target object whose `menuPicked:` records the chosen tag. Its class
/// is registered once.
fn handler() -> Id {
    static INIT: Once = Once::new();
    static mut HANDLER: Id = std::ptr::null_mut();
    INIT.call_once(|| unsafe {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("ClipCatMenuHandler", superclass)
            .expect("register ClipCatMenuHandler");
        decl.add_method(
            sel!(menuPicked:),
            menu_picked as extern "C" fn(&Object, Sel, Id),
        );
        let cls = decl.register();
        HANDLER = msg_send![cls, new];
    });
    unsafe { HANDLER }
}

fn nsstring(s: &str) -> Id {
    let bytes = s.as_bytes();
    unsafe {
        let cls = class!(NSString);
        msg_send![cls,
            stringWithBytes: bytes.as_ptr()
            length: bytes.len()
            encoding: 4usize] // NSUTF8StringEncoding
    }
}

fn ns_view(window: &Window) -> Option<Id> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr() as Id),
        _ => None,
    }
}

/// Shows `labels` as a context menu at the current cursor position and returns
/// the index of the chosen item, or `None` if dismissed.
pub fn popup(window: &Window, labels: &[String]) -> Option<usize> {
    if labels.is_empty() {
        return None;
    }
    let view = ns_view(window)?;
    SELECTED.store(-1, Ordering::SeqCst);
    let null: Id = std::ptr::null_mut();
    unsafe {
        let pool: Id = msg_send![class!(NSAutoreleasePool), new];

        let menu: Id = msg_send![class!(NSMenu), alloc];
        let menu: Id = msg_send![menu, init];
        let _: () = msg_send![menu, setAutoenablesItems: NO];

        let target = handler();
        let empty = nsstring("");
        for (i, label) in labels.iter().enumerate() {
            let title = nsstring(label);
            let item: Id = msg_send![class!(NSMenuItem), alloc];
            let item: Id = msg_send![item,
                initWithTitle: title
                action: sel!(menuPicked:)
                keyEquivalent: empty];
            let _: () = msg_send![item, setTag: i as isize];
            let _: () = msg_send![item, setTarget: target];
            let _: () = msg_send![menu, addItem: item];
            let _: () = msg_send![item, release];
        }

        // Cursor in window base coords → view coords, then pop up there.
        let ns_window: Id = msg_send![view, window];
        let win_pt: CGPoint = msg_send![ns_window, mouseLocationOutsideOfEventStream];
        let view_pt: CGPoint = msg_send![view, convertPoint: win_pt fromView: null];
        let _: BOOL = msg_send![menu,
            popUpMenuPositioningItem: null
            atLocation: view_pt
            inView: view];

        let _: () = msg_send![menu, release];
        let _: () = msg_send![pool, drain];
    }

    match SELECTED.load(Ordering::SeqCst) {
        i if i >= 0 => Some(i as usize),
        _ => None,
    }
}
