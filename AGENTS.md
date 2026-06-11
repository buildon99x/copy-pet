# AGENTS.md — DeskCat contributor & agent guide

DeskCat is a tiny, dependency-light desktop pet (a Bongo-Cat-style typing
companion) written in Rust. A cat sits at the bottom of the screen, taps along
with your keyboard/mouse, earns XP, levels up and unlocks accessories, and shows
today's activity stats. Primary target is **Windows** (premium native build);
**macOS** and **Linux** are supported via a portable backend.

> This file is the single source of truth for working in this repo. `CLAUDE.md`
> imports it. Keep it accurate when structure or commands change.

## Golden rules

1. **Privacy is non-negotiable.** Global input hooks may only increment the
   atomic counters in `src/input.rs`. Never read, store, log or transmit key
   contents, characters, window titles or timings. There is no network code —
   keep it that way.
2. **Keep the core platform-agnostic.** Simulation, rendering, progression and
   persistence live in the core and must not reference any OS API. OS code lives
   only under `src/platform/`.
3. **No new heavy dependencies** without an ADR. The whole point is a ~0.6 MB
   binary with a handful of crates and no asset pipeline (icon, font and sounds
   are generated from code).
4. **Verify by running, on release.** Build, launch, and screenshot the real app
   (see *Verifying*). Benchmark CPU only on `--release` (see
   [LNR-0002](.context/kb/lnr/0002-debug-vs-release-cpu.md)).

## Repository layout

```
src/
  main.rs              thin entry → deskcat::platform::run()
  lib.rs               module wiring
  pet.rs               Pet: the platform-agnostic simulation + scene building
  render.rs            all vector art (cat, accessories, particles, bubble, icon)
  font.rs              built-in 5×7 pixel font (no font files)
  sound.rs             synthesized SFX; winmm on Windows, no-op elsewhere
  state.rs             Persist (JSON) + XP/level progression + accessory table
  input.rs             shared atomic activity counters (KEYS/CLICKS/WHEEL)
  platform/
    mod.rs             selects exactly one backend by cfg
    windows.rs         native Win32 layered window + LL hooks + Shell tray
    portable.rs        winit + softbuffer + rdev (macOS/Linux, and Win --feature)
  bin/gen_icon.rs      regenerates assets/deskcat.ico from render::draw_icon_scaled
build.rs               embeds icon + version info on Windows hosts
assets/deskcat.ico     the embedded app icon — generated, not hand-edited
assets/screenshot.png  README screenshot (the only committed image asset)
docs/specs/            product & technical specs
.context/kb/adr/       architecture decision records (why)
.context/kb/lnr/       lessons & near-misses (what bit us)
.github/workflows/     CI: builds on windows + macos + ubuntu
```

## Architecture

Two backends share one core; exactly one backend compiles per build, chosen in
`platform/mod.rs`:

- `all(windows, not(feature = "portable"))` → **native Win32** (default on
  Windows, the release target): per-pixel alpha + click-through layered window,
  `WH_*_LL` hooks, Shell tray with full context menu, HKCU autostart.
- `any(not(windows), feature = "portable"))` → **portable**: `winit` window +
  `softbuffer` present + `rdev` global input. Pet drawn on an opaque card;
  settings via keyboard shortcuts.

The core flow: a ~33 ms tick drains `input` counters → `Pet::advance(k,c,wh)`
updates animation/XP/particles → `Pet::render*` builds a `Scene` and rasterizes
with tiny-skia → the backend presents the pixel buffer. The Pet never touches
the OS; the backend never touches simulation internals (only the public `Pet`
API). Read [ADR-0001](.context/kb/adr/0001-cross-platform-architecture.md) first.

## Commands

```bash
# Build / run (default backend: native on Windows, portable on macOS/Linux)
cargo build --release
cargo run --release

# Run the PORTABLE backend on Windows (to test the cross-platform path locally)
cargo run --features portable
cargo build --release --features portable

# Quality gates (run all three before calling something done)
cargo clippy --release                      # keep clean
cargo clippy --release --features portable  # on Windows, also lint portable path
cargo test --release                        # core logic unit tests

# Regenerate the icon after changing the cat art in render.rs
cargo run --bin gen_icon
```

macOS/Linux need system libs for the portable stack — see the CI workflow's
`Install Linux system dependencies` step for the exact apt list.

## Coding conventions

- Match the surrounding style; module-level `//!` docs explain the "why".
- Core code is `#![forbid]`-clean of OS calls; platform specifics use
  `#[cfg(...)]`, never runtime OS detection.
- Prefer generating assets in code over bundling files.
- `unsafe` is confined to `platform/windows.rs` (Win32 FFI) and the small WAV/
  icon byte-buffer builders; document the safety invariant inline.
- Keep both backends' interaction set in parity (drag, single-click bounce,
  double-click pet, hover stats). If you add an interaction, add it to both.

## Knowledge base — when to write what

- **ADR** (`.context/kb/adr/NNNN-title.md`): a significant, hard-to-reverse
  decision (a new dependency, a backend, a data-format change). Context →
  Decision → Consequences. Add a row to the ADR index.
- **LNR** (`.context/kb/lnr/NNNN-title.md`): something that cost real
  debugging time and is easy to repeat. Symptom → Cause → Fix → Takeaway.
- **Spec** (`docs/specs/`): product/technical behavior of a feature.

Convert relative dates to absolute when writing these. Link related records with
relative markdown links.

## Verifying a change (Windows dev machine)

1. `cargo build --release` (and `--features portable` for the portable path).
2. Launch the exe, then screenshot the bottom-right of the screen to confirm the
   pet renders correctly (transparent on native; on a card on portable).
3. Confirm CPU is a few percent and memory ~12–16 MB on release.
4. macOS/Linux runtime is validated by CI builds + code review, not locally
   (only the MSVC toolchain is installed here) — say so honestly in summaries.

## Gotchas (see LNR for detail)

- softbuffer can't do per-pixel desktop alpha → portable uses an opaque card.
- Debug builds render ~10× slower; never judge CPU off a debug run.
- `rdev::listen` blocks → it runs on its own thread; macOS needs Accessibility
  permission, Wayland blocks global capture.
- The dual optional/`[target.'cfg(...)']` dependency layout is deliberate; don't
  "simplify" it without re-reading
  [LNR-0004](.context/kb/lnr/0004-cargo-cross-platform-deps.md).
