# ADR-0002: winmm audio on Windows, silent no-op elsewhere (v1)

- Status: Accepted
- Date: 2026-06-12

## Context

Sound effects (paw taps, level-up chime, pet "boop") are synthesized in memory
and played with zero assets. On Windows that is a one-call `PlaySound` (winmm).
Cross-platform playback would mean a stack like `cpal`/`rodio`, which is a heavy
dependency tree, adds startup cost, and — critically — cannot be runtime-tested
on our Windows-only dev machine.

## Decision

Keep the public sound API (`sound::{init, play_tap, play_chime, play_pop}`)
identical on all platforms. Implement it with **winmm on Windows** and a
**silent no-op on macOS/Linux** for v1. The synthesis code and the API are
structured so a portable audio backend can be slotted behind the same four
functions later without touching callers.

## Consequences

- ✅ Lean, robust builds everywhere; no untested audio stack on macOS/Linux.
- ✅ Sound logic in `pet::Pet` is unchanged across platforms (it just calls the
  API; the no-op absorbs it).
- ⚠️ No sound on macOS/Linux in v1 — documented in the README and spec.
- 🔜 Follow-up: a `rodio`-backed implementation gated behind a feature, if/when
  there is a way to validate it on those platforms.
