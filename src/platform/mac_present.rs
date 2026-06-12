//! macOS transparent presentation.
//!
//! softbuffer can only deliver *opaque* pixels (ADR-0003 / LNR-0001), so the
//! portable backend normally draws the pet on an opaque "card". On macOS we
//! instead drive the window's `CALayer` directly: each frame the tiny-skia
//! pixmap — which already carries premultiplied alpha — is wrapped in a
//! `CGImage` and set as `layer.contents`, over a non-opaque, shadowless
//! `NSWindow` with a clear background. The result is a free-floating,
//! background-transparent pet like the native Windows layered window.
//!
//! Interactions are unchanged: the window still receives events over its whole
//! rectangle (this path is *visually* transparent, not click-through).
//!
//! All `unsafe` here is Objective-C messaging / CoreGraphics FFI. Invariants:
//! `view`/`layer` are owned by the live `winit` window we were built from and
//! are only messaged on the main thread while that window is alive; the
//! per-frame `CGImage` is released when it drops, after `setContents:`
//! retains it.

use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use foreign_types::ForeignType;
use objc::runtime::{Object, NO, YES};
use objc::{class, msg_send, sel, sel_impl};
use tiny_skia::Pixmap;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// premultiplied RGBA, the byte order tiny-skia's pixmap already uses.
const K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST: u32 = 1;

type Id = *mut Object;

/// Presents pixmaps to one window's layer. Holds borrowed AppKit object
/// pointers — valid only while the owning window lives (it outlives us).
pub struct Presenter {
    layer: Id,
    color_space: CGColorSpace,
}

// The pointers are only ever touched on the main (UI) thread, like the rest of
// the winit window; the struct never crosses a thread boundary.
impl Presenter {
    /// Configures the window for transparency and returns a presenter, or
    /// `None` if the AppKit view/layer can't be obtained (then the caller
    /// falls back to softbuffer's opaque card).
    pub fn new(window: &Window) -> Option<Presenter> {
        let handle = window.window_handle().ok()?;
        let RawWindowHandle::AppKit(h) = handle.as_raw() else {
            return None;
        };
        let view = h.ns_view.as_ptr() as Id;
        unsafe {
            let ns_window: Id = msg_send![view, window];
            if !ns_window.is_null() {
                let _: () = msg_send![ns_window, setOpaque: NO];
                let clear: Id = msg_send![class!(NSColor), clearColor];
                let _: () = msg_send![ns_window, setBackgroundColor: clear];
                let _: () = msg_send![ns_window, setHasShadow: NO];
            }
            let _: () = msg_send![view, setWantsLayer: YES];
            let layer: Id = msg_send![view, layer];
            if layer.is_null() {
                return None;
            }
            let _: () = msg_send![layer, setOpaque: NO];
            Some(Presenter {
                layer,
                color_space: CGColorSpace::create_device_rgb(),
            })
        }
    }

    /// Pushes one rendered frame to the layer. `scale` is the window's backing
    /// scale factor so the physical-pixel image maps to the right point size on
    /// Retina displays.
    pub fn present(&mut self, pm: &Pixmap, scale: f64) {
        let w = pm.width() as usize;
        let h = pm.height() as usize;
        if w == 0 || h == 0 {
            return;
        }
        // Snapshot the premultiplied RGBA pixmap into an immutable CGImage.
        let mut ctx = CGContext::create_bitmap_context(
            None,
            w,
            h,
            8,
            w * 4,
            &self.color_space,
            K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST,
        );
        ctx.data().copy_from_slice(pm.data());
        let Some(image) = ctx.create_image() else {
            return;
        };

        unsafe {
            // No implicit fade animation between frames.
            let _: () = msg_send![class!(CATransaction), begin];
            let _: () = msg_send![class!(CATransaction), setDisableActions: YES];
            let _: () = msg_send![self.layer, setContentsScale: scale];
            let _: () = msg_send![self.layer, setContents: image.as_ptr()];
            let _: () = msg_send![class!(CATransaction), commit];
        }
        // `image` drops here (CFRelease); the layer retained it in setContents:.
    }
}
