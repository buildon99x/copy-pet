//! Global activity counters. Both platform backends feed these from their
//! respective global input hooks (Win32 `WH_*_LL` on Windows, `rdev` on the
//! portable backend). The hook callbacks only ever touch these atomics — they
//! never read which key was pressed and never lock app state, with one narrow
//! exception: [`key_down`]/[`key_up`] reduce the keycode to a bit in a
//! held-key bitmap so OS auto-repeat while a key is held does not inflate the
//! counters. The keycode is used as a bit index only and immediately
//! discarded — no identity, order, timing or content is ever stored, logged
//! or transmitted (ADR-0008 / golden rule 1).

use std::sync::atomic::{AtomicU32, Ordering};

pub static KEYS: AtomicU32 = AtomicU32::new(0);
pub static CLICKS: AtomicU32 = AtomicU32::new(0);
pub static WHEEL: AtomicU32 = AtomicU32::new(0);

/// Per-keycode "currently held" bitmap (256 bits) backing the auto-repeat
/// suppression in [`key_down`]/[`key_up`].
static KEY_HELD: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];

fn held_slot(code: u16) -> (usize, u32) {
    let i = (code as usize) & 0xFF;
    (i / 32, 1u32 << (i % 32))
}

/// Counts one key press without repeat information (used by backends that
/// gate auto-repeat themselves, like the portable `KeyGate`).
pub fn key() {
    KEYS.fetch_add(1, Ordering::Relaxed);
}

/// A global key-down event with its keycode: counts only when the key was
/// not already held, so holding a key (OS auto-repeat) counts once.
pub fn key_down(code: u16) {
    let (slot, bit) = held_slot(code);
    if KEY_HELD[slot].fetch_or(bit, Ordering::Relaxed) & bit == 0 {
        key();
    }
}

/// The matching key-up: the next press of this keycode counts again.
pub fn key_up(code: u16) {
    let (slot, bit) = held_slot(code);
    KEY_HELD[slot].fetch_and(!bit, Ordering::Relaxed);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// One test (not several) so the shared counters aren't drained
    /// concurrently by parallel test threads.
    #[test]
    fn auto_repeat_counts_once_per_hold() {
        let _ = drain();
        key_down(0x41);
        key_down(0x41); // OS auto-repeat while held
        key_down(0x41);
        key_down(0x42); // a second key held alongside
        assert_eq!(drain().0, 2, "held keys count once");
        key_up(0x41);
        key_down(0x41); // released and pressed again: counts again
        assert_eq!(drain().0, 1);
        key_up(0x41);
        key_up(0x42);
        // keycodes beyond the bitmap fold into it instead of panicking
        key_down(0x1FF);
        key_up(0x1FF);
        assert_eq!(drain().0, 1);
    }
}
