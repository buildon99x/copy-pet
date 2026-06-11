# LNR-0004: "optional on Windows, required elsewhere" dependency layout

- Date: 2026-06-12 · Area: build / cargo

## Goal

The portable crates (`winit`, `softbuffer`, `rdev`) must be:
- **not compiled** on a default Windows build (keep the native binary tiny),
- **opt-in** on Windows via `--features portable` (to test the portable path),
- **always compiled** on macOS/Linux with a plain `cargo build` (no flag).

## Solution

Declare them **optional** in `[dependencies]` and have the `portable` feature
enable them (`portable = ["dep:winit", "dep:softbuffer", "dep:rdev"]`), then
**also** list them as plain dependencies under
`[target.'cfg(not(windows))'.dependencies]`. Cargo unifies the two declarations:
optional+feature-gated on Windows, unconditional on non-Windows.

Windows-only crates go under `[target.'cfg(windows)'.dependencies]`
(`windows-sys`) and `[target.'cfg(windows)'.build-dependencies]`
(`winresource`); `libc` (for `localtime_r`) under `[target.'cfg(unix)']`.

## Gotchas

- `Cargo.lock` resolves dependencies for **all** targets, so building on Windows
  still *adds* the Linux-only transitive crates to the lockfile. They are not
  *compiled* unless their target/feature is active — this is expected, not a
  leak into the Windows binary.
- `build.rs` runs on the **host**; gate `winresource` use with `#[cfg(windows)]`
  so a non-Windows host build doesn't try to reference a crate it doesn't have.

## Takeaway

Per-target + optional/feature dependency tables compose cleanly. Verify the
intended split with `cargo tree --target <triple>` / `--features` rather than
trusting the lockfile contents.
