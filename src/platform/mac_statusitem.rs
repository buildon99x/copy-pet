//! macOS menu-bar status item — an `NSStatusItem` in the system menu bar.
//!
//! The portable backend hides the Dock icon (Accessory activation policy), so
//! the menu bar is ClipCat's always-available surface: clicking the item opens
//! the same context menu as a right-click on the pet — and it's the only way
//! back when the pet is hidden. The button shows a monochrome **template**
//! image of the cat (rendered from [`crate::render::draw_icon`]), so AppKit
//! tints it for light/dark menu bars automatically.
//!
//! Like the rest of the portable macOS path this runs on the main (winit)
//! thread. We deliberately don't attach an `NSMenu` to the item (`setMenu:`):
//! the menu must be rebuilt from live `Pet` state and the chosen action applied
//! through `&mut Pet`, which an objc menu-delegate callback can't reach. Instead
//! the button's click only flips a `CLICKED` atomic, and the main loop's
//! `about_to_wait` tick reads it and runs the existing menu flow — the same
//! atomic-signal pattern as the global panel hotkey.
//!
//! All `unsafe` is Objective-C messaging / CoreGraphics FFI. The handler class
//! is registered once; the status item is `retain`ed for the process lifetime
//! (it lives in the menu bar until exit), mirroring `mac_menu`'s static handler.

#![allow(unexpected_cfgs)] // objc 0.2's msg_send!/class!/sel! macros

use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::geometry::CGSize;
use foreign_types::ForeignType;
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel, YES};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use tiny_skia::Pixmap;

use super::mac_util::{nsstring, Id};

/// premultiplied RGBA — the byte order tiny-skia's pixmap uses (matches
/// `mac_present`).
const K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST: u32 = 1;

/// `NSVariableStatusItemLength` — size the item to fit its image.
const NS_VARIABLE_STATUS_ITEM_LENGTH: f64 = -1.0;

/// Set when the status-item button is clicked; drained by `about_to_wait`.
static CLICKED: AtomicBool = AtomicBool::new(false);

/// Returns (and clears) whether the menu-bar item was clicked since the last
/// tick.
pub fn take_clicked() -> bool {
    CLICKED.swap(false, Ordering::SeqCst)
}

extern "C" fn status_clicked(_this: &Object, _cmd: Sel, _sender: Id) {
    CLICKED.store(true, Ordering::SeqCst);
}

/// A shared target whose `statusClicked:` flips `CLICKED`. Registered once.
fn handler() -> Id {
    static INIT: Once = Once::new();
    static mut HANDLER: Id = std::ptr::null_mut();
    INIT.call_once(|| unsafe {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("ClipCatStatusHandler", superclass)
            .expect("register ClipCatStatusHandler");
        decl.add_method(
            sel!(statusClicked:),
            status_clicked as extern "C" fn(&Object, Sel, Id),
        );
        let cls = decl.register();
        HANDLER = msg_send![cls, new];
    });
    unsafe { HANDLER }
}

/// Renders the cat icon as a monochrome **template** `NSImage` (18pt) so AppKit
/// tints it for the active menu-bar appearance. Reuses `mac_present`'s pixmap →
/// `CGImage` recipe; the cat's alpha coverage becomes the tinted silhouette.
fn template_image() -> Id {
    let mut pm = Pixmap::new(32, 32).expect("32x32 pixmap");
    crate::render::draw_icon(&mut pm);
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let color_space = CGColorSpace::create_device_rgb();
    let mut ctx = CGContext::create_bitmap_context(
        None,
        w,
        h,
        8,
        w * 4,
        &color_space,
        K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST,
    );
    ctx.data().copy_from_slice(pm.data());
    let image = ctx.create_image().expect("CGImage from pixmap");
    unsafe {
        let size = CGSize::new(18.0, 18.0);
        let ns_image: Id = msg_send![class!(NSImage), alloc];
        let ns_image: Id = msg_send![ns_image, initWithCGImage: image.as_ptr() size: size];
        let _: () = msg_send![ns_image, setTemplate: YES];
        ns_image
    }
    // `image` (CGImage) drops here; `NSImage` made its own copy of the bytes.
}

/// Installs the menu-bar status item: a template cat icon whose click flips the
/// `CLICKED` flag. Call once (from `resumed`). The item is retained for the
/// process lifetime — it stays in the menu bar until the app exits.
pub fn create() {
    unsafe {
        let bar: Id = msg_send![class!(NSStatusBar), systemStatusBar];
        let item: Id = msg_send![bar, statusItemWithLength: NS_VARIABLE_STATUS_ITEM_LENGTH];
        if item.is_null() {
            return;
        }
        let _: Id = msg_send![item, retain];

        let button: Id = msg_send![item, button];
        if button.is_null() {
            return;
        }
        let image = template_image();
        let _: () = msg_send![button, setImage: image];
        let _: () = msg_send![image, release]; // button retained it via setImage:
        let _: () = msg_send![button, setToolTip: nsstring("ClipCat")];
        let _: () = msg_send![button, setTarget: handler()];
        let _: () = msg_send![button, setAction: sel!(statusClicked:)];
    }
}
