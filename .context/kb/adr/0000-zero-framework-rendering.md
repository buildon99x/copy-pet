# ADR-0000: Zero-framework rendering (raw platform APIs + tiny-skia)

- Status: Accepted
- Date: 2026-06-11

## Context

DeskCat is a small, always-on desktop ornament. It must feel native and
lightweight (tiny binary, low memory, no install), and it needs a *shaped*,
transparent, click-through window — something most GUI toolkits make awkward or
heavy. We also want full control over the hand-drawn vector look.

## Decision

Use **no GUI framework**. Draw everything ourselves with
[`tiny-skia`](https://crates.io/crates/tiny-skia) (CPU vector rasterizer) onto a
pixel buffer, and talk to the OS directly:

- The cat, desk, keyboard, accessories, particles, stats bubble and toasts are
  all vector art in `render.rs`.
- Text uses a hand-built 5×7 pixel font (`font.rs`) — no font files.
- Sound effects are synthesized in memory at startup (`sound.rs`) — no assets.
- The tray/window icon is generated from the same art (`bin/gen_icon.rs`).

## Consequences

- ✅ ~0.6 MB single binary, ~12–16 MB RAM, a handful of dependencies.
- ✅ Complete control of the aesthetic and the window shape/transparency.
- ✅ No asset pipeline; the repo carries only one generated `.ico`.
- ⚠️ We own all windowing/input plumbing per platform — the motivation for the
  backend split in [ADR-0001](0001-cross-platform-architecture.md).
- ⚠️ Rendering is CPU-side every frame; cost is acceptable in release builds but
  conspicuous in debug (see [LNR-0002](../lnr/0002-debug-vs-release-cpu.md)).
