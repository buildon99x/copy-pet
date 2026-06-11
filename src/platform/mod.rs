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
