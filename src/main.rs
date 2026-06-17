//! ClipCat — a desktop cat that manages your clipboard.
//!
//! The simulation, clipboard store and rendering are platform-agnostic
//! (`clipcat::pet`, `clipcat::clipboard`, `clipcat::render`); the OS event
//! loop, window, clipboard watcher and global input hooks live in
//! `clipcat::platform`, which selects the native Win32 backend on Windows
//! and a portable `winit`/`softbuffer`/`rdev`/`arboard` backend on macOS
//! and Linux.

// Hide the console window on release Windows builds (GUI subsystem).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    // Record panics (message + backtrace) to <config_dir>/clipcat.log so a
    // crash on launch leaves a trace the user can hand back — chasing the
    // macOS-15 startup crash (see clipcat::diag).
    clipcat::diag::install_panic_hook();
    clipcat::platform::run();
}
