//! macOS right-click context menu — a native `NSMenu` rendered from the
//! platform-agnostic [`crate::menu`] model.
//!
//! The portable backend's window is a small, borderless layer; a self-drawn
//! menu can't extend past it, so on macOS we pop a real `NSMenu` at the cursor
//! (it draws in its own window, like the Windows tray menu). The menu only
//! *renders* the model — labels, check marks, disabled/locked items and
//! submenus; selecting an item returns the model's [`MenuAction`], which the
//! backend applies through `Pet::apply_menu_action`. No behavior lives here.
//!
//! All `unsafe` is Objective-C messaging. `popup` blocks in AppKit's nested
//! tracking run loop until the menu is dismissed; the chosen item's tag (an
//! index into the flat action list we build alongside the menu) lands in
//! `SELECTED` via the one registered handler, which we read back after.
//! `alloc`-ed objects are released; convenience temporaries are mopped up by
//! the autorelease pool around the popup.

#![allow(unexpected_cfgs)] // objc 0.2's msg_send!/class!/sel! macros

use crate::menu::{MenuAction, MenuEntry};
use core_graphics::geometry::CGPoint;
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel, BOOL, NO};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Once;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use super::mac_util::{nsstring, Id};

/// `NSControlStateValueOn` — a checked menu item.
const STATE_ON: isize = 1;

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

fn ns_view(window: &Window) -> Option<Id> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr() as Id),
        _ => None,
    }
}

/// Recursively fills `ns_menu` from `entries`, assigning each actionable item a
/// tag = its index in `actions` (the flat lookup the handler reports back).
unsafe fn build_into(ns_menu: Id, entries: &[MenuEntry], actions: &mut Vec<MenuAction>, target: Id, empty: Id) {
    let _: () = msg_send![ns_menu, setAutoenablesItems: NO];
    for entry in entries {
        match entry {
            MenuEntry::Separator => {
                let sep: Id = msg_send![class!(NSMenuItem), separatorItem];
                let _: () = msg_send![ns_menu, addItem: sep];
            }
            MenuEntry::Item(item) => {
                let title = nsstring(&item.label);
                if !item.submenu.is_empty() {
                    let mi: Id = msg_send![class!(NSMenuItem), alloc];
                    let mi: Id = msg_send![mi, init];
                    let _: () = msg_send![mi, setTitle: title];
                    let child: Id = msg_send![class!(NSMenu), alloc];
                    let child: Id = msg_send![child, init];
                    build_into(child, &item.submenu, actions, target, empty);
                    let _: () = msg_send![mi, setSubmenu: child];
                    let _: () = msg_send![ns_menu, addItem: mi];
                    let _: () = msg_send![mi, release];
                    let _: () = msg_send![child, release];
                } else {
                    let mi: Id = msg_send![class!(NSMenuItem), alloc];
                    let mi: Id = msg_send![mi,
                        initWithTitle: title
                        action: sel!(menuPicked:)
                        keyEquivalent: empty];
                    if let Some(action) = item.action {
                        let _: () = msg_send![mi, setTarget: target];
                        let _: () = msg_send![mi, setTag: actions.len() as isize];
                        actions.push(action);
                    }
                    if item.checked {
                        let _: () = msg_send![mi, setState: STATE_ON];
                    }
                    if !item.enabled {
                        let _: () = msg_send![mi, setEnabled: NO];
                    }
                    let _: () = msg_send![ns_menu, addItem: mi];
                    let _: () = msg_send![mi, release];
                }
            }
        }
    }
}

/// Shows `entries` as a context menu at the current cursor and returns the
/// chosen [`MenuAction`], or `None` if dismissed.
pub fn popup(window: &Window, entries: &[MenuEntry]) -> Option<MenuAction> {
    if entries.is_empty() {
        return None;
    }
    let view = ns_view(window)?;
    SELECTED.store(-1, Ordering::SeqCst);
    let null: Id = std::ptr::null_mut();
    let mut actions: Vec<MenuAction> = Vec::new();
    unsafe {
        let pool: Id = msg_send![class!(NSAutoreleasePool), new];

        let menu: Id = msg_send![class!(NSMenu), alloc];
        let menu: Id = msg_send![menu, init];
        let empty = nsstring("");
        build_into(menu, entries, &mut actions, handler(), empty);

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
        i if i >= 0 => actions.get(i as usize).copied(),
        _ => None,
    }
}

/// Like [`popup`] but positioned at the global cursor in screen coordinates
/// (`inView: nil`) instead of relative to a window/view — used by the menu-bar
/// status item, whose click drops the menu just under its icon. Shares
/// everything else (`build_into`/`handler`/`SELECTED`) with [`popup`].
pub fn popup_at_cursor(entries: &[MenuEntry]) -> Option<MenuAction> {
    if entries.is_empty() {
        return None;
    }
    SELECTED.store(-1, Ordering::SeqCst);
    let null: Id = std::ptr::null_mut();
    let mut actions: Vec<MenuAction> = Vec::new();
    unsafe {
        let pool: Id = msg_send![class!(NSAutoreleasePool), new];

        let menu: Id = msg_send![class!(NSMenu), alloc];
        let menu: Id = msg_send![menu, init];
        let empty = nsstring("");
        build_into(menu, entries, &mut actions, handler(), empty);

        // Current cursor in screen coords; inView: nil pops there on screen.
        let at: CGPoint = msg_send![class!(NSEvent), mouseLocation];
        let _: BOOL = msg_send![menu,
            popUpMenuPositioningItem: null
            atLocation: at
            inView: null];

        let _: () = msg_send![menu, release];
        let _: () = msg_send![pool, drain];
    }

    match SELECTED.load(Ordering::SeqCst) {
        i if i >= 0 => actions.get(i as usize).copied(),
        _ => None,
    }
}
