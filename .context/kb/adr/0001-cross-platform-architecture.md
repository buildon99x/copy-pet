# ADR-0001: Native Windows backend + portable backend over a shared core

- Status: Accepted
- Date: 2026-06-12
- Supersedes: the Windows-only scope of v1.0

## Context

v1.0 shipped a polished, fully native Win32 implementation (per-pixel alpha,
click-through, LL input hooks, Shell tray). We then needed macOS and Linux
support. The Win32 niceties (especially per-pixel click-through and the Shell
tray) have no portable equivalent, and a full rewrite onto a cross-platform
toolkit would regress the premium Windows experience that is the primary target.

## Decision

Split into a **platform-agnostic core** plus **two interchangeable backends**,
exactly one compiled per build:

- Core (`pet::Pet`, `render`, `state`, `font`, `sound`, `input`): the entire
  simulation, progression, rendering and persistence. No windowing/input/OS UI.
- `platform/windows.rs`: the existing native Win32 shell. Default on Windows.
- `platform/portable.rs`: `winit` + `softbuffer` + `rdev`. Default on
  macOS/Linux; selectable on Windows via the `portable` cargo feature.

`platform/mod.rs` selects via cfg:
`all(windows, not(feature = "portable"))` → windows; `any(not(windows),
feature = "portable")` → portable.

The `portable` feature lets the portable backend be **built and run on the dev
machine (Windows)**, which is how it is verified locally; GitHub Actions builds
the real native targets on all three OSes.

## Consequences

- ✅ Windows keeps its best-in-class native look and feel, unchanged.
- ✅ Core logic is OS-free, unit-testable, and identical across platforms.
- ✅ The portable path is exercised on every Windows dev build and in CI, so it
  cannot silently rot.
- ⚠️ Two backends to maintain; interaction parity must be kept in sync by hand.
- ⚠️ Some native-only affordances are intentionally absent on portable — see
  [ADR-0003](0003-portable-rendering-card.md) and
  [ADR-0004](0004-portable-settings-keyboard.md).
- ⚠️ macOS/Linux **runtime** behavior is validated by CI builds and review, not
  by local execution on the dev machine (Windows-only toolchain installed).
