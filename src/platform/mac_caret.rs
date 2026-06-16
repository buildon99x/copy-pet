//! macOS caret-position read for the clipboard flyout (Win+V parity).
//!
//! Returns where to anchor the flyout panel: the focused UI element's text
//! caret, via the Accessibility API (`AXUIElement`). Reads **geometry only** —
//! the caret's screen rectangle — never the element's text, value, title or
//! any other content (privacy: golden rule #1; the same posture as the input
//! event tap in [`super::mac_input`]). Falls back to the mouse-cursor location
//! when the focused app exposes no AX caret (many Electron/Java apps), so the
//! flyout always has somewhere to open.
//!
//! Output is in **physical pixels, top-left origin** — the same space winit's
//! `PhysicalPosition` and `MonitorHandle` use — so the portable backend can
//! place the flyout window without further conversion. The points→pixels
//! scale uses the main screen's backing factor; on a mixed-DPI secondary
//! monitor that is approximate (acceptable: the monitor-fit still keeps the
//! card on screen).
//!
//! All `unsafe` here is Objective-C / CoreFoundation / ApplicationServices
//! FFI. The AX system-wide element and every CFType we copy out are released
//! before returning (CoreFoundation "Copy" ownership rule).

use core_graphics::geometry::{CGPoint, CGRect};
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_void, CString};

type CFTypeRef = *const c_void;
type AXUIElementRef = CFTypeRef;
type CFStringRef = CFTypeRef;
type Id = *mut Object;

const AX_SUCCESS: i32 = 0;
/// `kAXValueCGRectType` — the AXValue wrapper kind for a `CGRect`.
const AX_VALUE_CGRECT: u32 = 3;
/// `kAXValueCFRangeType` — the AXValue wrapper kind for a `CFRange`.
const AX_VALUE_CFRANGE: u32 = 4;
/// `kCFStringEncodingUTF8`.
const CF_UTF8: u32 = 0x0800_0100;

/// `CFRange` — a `(location, length)` pair of `CFIndex` (pointer-sized signed).
/// The payload of an `AXSelectedTextRange` value.
#[repr(C)]
#[derive(Clone, Copy)]
struct CFRange {
    location: isize,
    length: isize,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    /// Whether this process is trusted for the Accessibility API. The AX caret
    /// read returns nothing without it (the input event tap needs the same
    /// trust, so it is normally already granted).
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        result: *mut CFTypeRef,
    ) -> i32;
    fn AXValueGetValue(value: CFTypeRef, the_type: u32, out: *mut c_void) -> bool;
    /// Wraps a plain value (here a `CFRange`) back into an `AXValue` so it can
    /// be passed as the parameter to `AXBoundsForRange`.
    fn AXValueCreate(the_type: u32, value_ptr: *const c_void) -> CFTypeRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        alloc: CFTypeRef,
        c_str: *const i8,
        encoding: u32,
    ) -> CFStringRef;
    fn CFRelease(cf: CFTypeRef);
}

/// A CFString for `name`, or null. Built from the literal attribute name
/// (e.g. "AXFocusedUIElement") so we don't depend on the `kAX*` exported
/// statics resolving at link time.
unsafe fn cfstr(name: &str) -> CFStringRef {
    let Ok(c) = CString::new(name) else {
        return std::ptr::null();
    };
    CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), CF_UTF8)
}

/// Screen-pixel anchor for the flyout (top-left origin): the focused app's
/// text caret bottom-left so the panel drops below the caret (Win+V style),
/// or the mouse cursor when no AX caret is available.
pub fn caret_screen_pos() -> Option<(f64, f64)> {
    unsafe { caret_via_ax() }.or_else(|| unsafe { mouse_location() })
}

unsafe fn caret_via_ax() -> Option<(f64, f64)> {
    if !AXIsProcessTrusted() {
        dbg_caret("not Accessibility-trusted; using mouse fallback");
        return None;
    }

    let attr_focused = cfstr("AXFocusedUIElement");
    let attr_range = cfstr("AXSelectedTextRange");
    let attr_bounds = cfstr("AXBoundsForRange");
    // guard: bail (releasing what we made) if any name failed to build
    let cleanup_names = |a: CFStringRef, b: CFStringRef, c: CFStringRef| {
        for s in [a, b, c] {
            if !s.is_null() {
                CFRelease(s);
            }
        }
    };
    if attr_focused.is_null() || attr_range.is_null() || attr_bounds.is_null() {
        cleanup_names(attr_focused, attr_range, attr_bounds);
        return None;
    }

    let sys = AXUIElementCreateSystemWide();
    let mut found = None;

    let mut focused: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(sys, attr_focused, &mut focused) == AX_SUCCESS
        && !focused.is_null()
    {
        let mut range: CFTypeRef = std::ptr::null();
        if AXUIElementCopyAttributeValue(focused, attr_range, &mut range) == AX_SUCCESS
            && !range.is_null()
        {
            // Bounds of the selection range — works directly when text is
            // selected. For a bare caret (zero-length range) several apps
            // refuse AXBoundsForRange, so retry one character wide at the same
            // location and use that glyph's left edge.
            found = bounds_origin(focused, attr_bounds, range);
            if found.is_none() {
                if let Some(wide) = widen_empty_range(range) {
                    found = bounds_origin(focused, attr_bounds, wide);
                    CFRelease(wide);
                }
            }
            CFRelease(range);
        } else {
            dbg_caret("no AXSelectedTextRange on the focused element");
        }
        CFRelease(focused);
    } else {
        dbg_caret("no AXFocusedUIElement");
    }
    CFRelease(sys);
    cleanup_names(attr_focused, attr_range, attr_bounds);
    if found.is_none() {
        dbg_caret("AX caret unavailable; using mouse fallback");
    }
    found
}

/// Bottom-left of the caret/selection rectangle for `range` (an AXValue), in
/// physical pixels (top-left origin), or `None` if the app won't report bounds
/// for it. Geometry only — the range's *text* is never read.
unsafe fn bounds_origin(
    element: CFTypeRef,
    attr_bounds: CFStringRef,
    range: CFTypeRef,
) -> Option<(f64, f64)> {
    let mut rect = CGRect::new(
        &CGPoint::new(0.0, 0.0),
        &core_graphics::geometry::CGSize::new(0.0, 0.0),
    );
    let mut bounds: CFTypeRef = std::ptr::null();
    let mut out = None;
    if AXUIElementCopyParameterizedAttributeValue(element, attr_bounds, range, &mut bounds)
        == AX_SUCCESS
        && !bounds.is_null()
    {
        if AXValueGetValue(bounds, AX_VALUE_CGRECT, &mut rect as *mut _ as *mut c_void)
            && rect.size.height > 0.0
        {
            // AX bounds are screen points, top-left origin. Anchor the caret's
            // bottom-left so the panel drops below it.
            let scale = main_scale();
            dbg_caret(&format!(
                "caret rect x={} y={} w={} h={}",
                rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
            ));
            out = Some((
                rect.origin.x * scale,
                (rect.origin.y + rect.size.height) * scale,
            ));
        }
        CFRelease(bounds);
    }
    out
}

/// A one-character AXValue range at the same location as `range`, when `range`
/// is an empty (caret) selection — else `None`. The returned AXValue is owned
/// by the caller (release with `CFRelease`).
unsafe fn widen_empty_range(range: CFTypeRef) -> Option<CFTypeRef> {
    let mut r = CFRange {
        location: 0,
        length: 0,
    };
    if !AXValueGetValue(range, AX_VALUE_CFRANGE, &mut r as *mut _ as *mut c_void) {
        return None;
    }
    if r.length != 0 {
        return None; // already a real selection; nothing to widen
    }
    let wide = CFRange {
        location: r.location,
        length: 1,
    };
    let v = AXValueCreate(AX_VALUE_CFRANGE, &wide as *const _ as *const c_void);
    if v.is_null() {
        None
    } else {
        Some(v)
    }
}

/// One-line diagnostic, emitted only when `CLIPCAT_DEBUG_CARET` is set in the
/// environment. Step names and geometry only — never element text (golden
/// rule #1). Off and silent by default.
fn dbg_caret(msg: &str) {
    if std::env::var_os("CLIPCAT_DEBUG_CARET").is_some() {
        eprintln!("[clipcat caret] {msg}");
    }
}

/// Backing scale factor of the main screen (1.0 fallback). Used to turn AX /
/// AppKit points into the physical pixels winit positions windows in.
unsafe fn main_scale() -> f64 {
    let screen: Id = msg_send![class!(NSScreen), mainScreen];
    if screen.is_null() {
        return 1.0;
    }
    let s: f64 = msg_send![screen, backingScaleFactor];
    if s > 0.0 {
        s
    } else {
        1.0
    }
}

/// The mouse cursor in physical pixels, top-left origin. `NSEvent` reports it
/// in points with a bottom-left origin, so flip by the main screen height.
unsafe fn mouse_location() -> Option<(f64, f64)> {
    let screen: Id = msg_send![class!(NSScreen), mainScreen];
    if screen.is_null() {
        return None;
    }
    let frame: CGRect = msg_send![screen, frame];
    let scale: f64 = msg_send![screen, backingScaleFactor];
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let loc: CGPoint = msg_send![class!(NSEvent), mouseLocation];
    let y_top = frame.size.height - loc.y;
    Some((loc.x * scale, y_top * scale))
}
