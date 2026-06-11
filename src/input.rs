//! Global activity counters. Both platform backends feed these from their
//! respective global input hooks (Win32 `WH_*_LL` on Windows, `rdev` on the
//! portable backend). The hook callbacks only ever touch these atomics — they
//! never read which key was pressed and never lock app state.

use std::sync::atomic::{AtomicU32, Ordering};

pub static KEYS: AtomicU32 = AtomicU32::new(0);
pub static CLICKS: AtomicU32 = AtomicU32::new(0);
pub static WHEEL: AtomicU32 = AtomicU32::new(0);

pub fn key() {
    KEYS.fetch_add(1, Ordering::Relaxed);
}
pub fn click() {
    CLICKS.fetch_add(1, Ordering::Relaxed);
}
pub fn wheel() {
    WHEEL.fetch_add(1, Ordering::Relaxed);
}

/// Atomically drains and returns (keys, clicks, wheel) since the last call.
pub fn drain() -> (u64, u64, u64) {
    (
        KEYS.swap(0, Ordering::Relaxed) as u64,
        CLICKS.swap(0, Ordering::Relaxed) as u64,
        WHEEL.swap(0, Ordering::Relaxed) as u64,
    )
}
