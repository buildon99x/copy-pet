//! Platform backends. Exactly one is compiled:
//!
//! * **windows** — native Win32 layered window with per-pixel alpha and
//!   click-through, low-level input hooks and a Shell tray icon. The default
//!   on Windows and the release target.
//! * **portable** — `winit` + `softbuffer` window with `rdev` global input;
//!   settings are driven by keyboard shortcuts (no system tray — see ADR-0004).
//!   Used on macOS and Linux, and on Windows when the `portable` feature is
//!   enabled (so the portable path can be exercised on the development machine).
//!
//! Both expose a single `run()` entry point that owns the event loop.

#[cfg(all(windows, not(feature = "portable")))]
mod windows;
#[cfg(all(windows, not(feature = "portable")))]
pub use windows::run;

#[cfg(any(not(windows), feature = "portable"))]
mod portable;
#[cfg(any(not(windows), feature = "portable"))]
pub use portable::run;

// macOS-only helpers for the portable backend: a bespoke CoreGraphics event
// tap (replaces rdev's crashing keyboard listener, LNR-0005), a transparent
// CALayer presenter (replaces softbuffer's opaque card, ADR-0003), the native
// right-click context menu and its dialogs, plus LaunchAgent autostart.
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_util;
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_input;
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_present;
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_menu;
// menu-bar NSStatusItem: the Dock icon is hidden (Accessory policy), so this is
// the always-available surface; a click opens the same mac_menu context menu.
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_statusitem;
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_dialogs;
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_autostart;
// caret-position read for the caret-anchored clipboard flyout (Win+V parity);
// geometry only, falls back to the mouse cursor.
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_caret;
// rich-format clipboard read/write via NSPasteboard (ADR-0014): original
// HTML/RTF preserved on copy and restored on paste, the macOS half of Win+V
// parity. The portable watcher reads it; `set_clipboard` writes it.
#[cfg(all(any(not(windows), feature = "portable"), target_os = "macos"))]
mod mac_clipboard;
