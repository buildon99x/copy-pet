//! DeskCat — a desktop cat that types along with you.
//!
//! The simulation and rendering are platform-agnostic (`deskcat::pet`,
//! `deskcat::render`); the OS event loop, window and global input hooks live
//! in `deskcat::platform`, which selects the native Win32 backend on Windows
//! and a portable `winit`/`softbuffer`/`rdev` backend on macOS and Linux.

// Hide the console window on release Windows builds (GUI subsystem).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    deskcat::platform::run();
}
