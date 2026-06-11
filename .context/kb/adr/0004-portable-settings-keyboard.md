# ADR-0004: Keyboard shortcuts instead of a tray menu on the portable backend

- Status: Accepted
- Date: 2026-06-12

## Context

On Windows the native backend exposes all settings through a rich Shell tray
context menu. The portable backend needs an equivalent. The obvious choice,
`tray-icon` + `muda`, integrates cleanly with `winit` on Windows/macOS but on
Linux requires a GTK main loop running alongside winit (extra dependency,
nontrivial integration, and not testable on our Windows-only dev machine).

## Decision

For the portable backend, **omit the system tray** and drive settings with
**keyboard shortcuts** while the window is focused (no extra dependencies, works
on all three OSes including Wayland):

`S` cycle size · `A` cycle unlocked accessory · `M` cycle sound mode ·
`B` toggle stats bubble · `L` toggle position lock · `Q`/`Esc` quit.

The window does not steal focus; clicking it focuses it so shortcuts apply.
Global typing still drives the core loop via `rdev` regardless of focus.

## Consequences

- ✅ Minimal dependency set (winit + softbuffer + rdev only); fully testable on
  Windows via `--features portable`.
- ✅ Works on Linux/Wayland where tray + global capture are unreliable.
- ⚠️ Less discoverable than a menu; mitigated by documenting shortcuts in the
  README, the spec, and the source header.
- ⚠️ "Reset stats" is intentionally **not** bound to a key (too destructive for
  an accidental press); it remains a native-Windows menu action for now.
- 🔜 Optional `tray-icon` menu for Windows/macOS could be added behind a feature.
