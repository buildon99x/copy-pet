//! Tiny, dependency-free diagnostic log for chasing startup crashes.
//!
//! Writes timestamped breadcrumbs and panic info to `<config_dir>/clipcat.log`
//! (see [`crate::state::config_dir`]). This exists because a hard crash on
//! launch — in particular a macOS hard `abort()` (SIGTRAP, e.g. a TIS call off
//! the main thread; see [LNR-0005](../.context/kb/lnr/0005-macos-tis-eventtap-crash.md))
//! that no `catch_unwind` or panic hook can intercept — otherwise leaves no
//! trace the user can hand back. The last breadcrumb with no line after it
//! localizes the crashing step; the panic hook captures ordinary Rust panics
//! with a backtrace.
//!
//! Best-effort only: every write failure is swallowed — logging must never take
//! the app down. Uses only `std`: no new dependency (golden rule #3) and no OS
//! call beyond [`crate::state::config_dir`] (golden rule #2).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

/// Process start, set on the first log call, so each breadcrumb carries a
/// `+NNNms` offset that orders the startup sequence.
static START: OnceLock<Instant> = OnceLock::new();

fn log_path() -> Option<PathBuf> {
    crate::state::config_dir().map(|d| d.join("clipcat.log"))
}

/// Appends one timestamped line to the diagnostic log. Silent on any failure.
pub fn log(msg: &str) {
    let start = *START.get_or_init(Instant::now);
    let Some(path) = log_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let ms = start.elapsed().as_millis();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "[+{ms:>7}ms] {msg}");
    }
}

/// Truncates the log and writes a fresh header (version, OS/arch, profile).
/// Call once at startup so the file holds only the most recent run.
pub fn init() {
    if let Some(path) = log_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, ""); // drop the previous run's lines
    }
    log(&format!(
        "ClipCat {} starting — os={} arch={} profile={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    ));
}

/// Installs a panic hook that records the panic message, location and a
/// backtrace to the diagnostic log before delegating to the previous hook.
/// Catches ordinary Rust panics; a hard `abort()` (e.g. macOS SIGTRAP) bypasses
/// this — that case is covered by the [`log`] breadcrumbs instead.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        log(&format!("PANIC at {loc}: {msg}"));
        log(&format!(
            "backtrace:\n{}",
            std::backtrace::Backtrace::force_capture()
        ));
        prev(info);
    }));
}
