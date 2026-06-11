# AGENTS.md — ClipCat contributor & agent guide

ClipCat is a tiny, dependency-light **clipboard-manager desktop pet** written
in Rust. A cat sits at the bottom of the screen and taps along with your
keyboard/mouse; every system-wide text copy is captured into a local, searchable
clipboard history — visualized as the cat eating a fish badged with the source
app. Copying, typing and clicking earn XP, level the cat up and unlock
accessories. The full UI is bilingual (English/Korean). Primary target is
**Windows** (premium native build); **macOS** and **Linux** are supported via a
portable backend.

> This file is the single source of truth for working in this repo. `CLAUDE.md`
> imports it. Keep it accurate when structure or commands change.

## Golden rules

1. **Privacy is non-negotiable.** Global *input* hooks may only increment the
   atomic counters in `src/input.rs`, with exactly one sanctioned exception:
   the portable backend's `ChordTracker` compares each key event against the
   user's configured panel-hotkey chord and immediately discards it
   (ADR-0008) — beyond that, never read, store, log or transmit key
   contents, window titles or timings. Clipboard *content* is the product,
   but it stays local: stored only in the user's config dir, capture can
   always be paused, and there is **no network code — keep it that way**.
2. **Keep the core platform-agnostic.** Simulation, clipboard store, panel
   logic, rendering, i18n, progression and persistence live in the core and
   must not reference any OS API (tiny per-OS leaves like `state::today_string`
   / `state::detect_lang` are the only sanctioned exceptions). OS code lives
   only under `src/platform/`.
3. **No new heavy dependencies** without an ADR. The whole point is a small
   binary with a handful of crates and no asset pipeline (icon, the built-in
   pixel/vector-Hangul fonts and sounds are generated from code; the UI font
   is read from the OS at runtime — never bundled, see ADR-0007).
4. **Verify by rendering/running, on release.** Build, run the tests, and
   eyeball `cargo run --release --example preview` PNGs (headless-friendly).
   On a Windows dev machine, also launch the exe and screenshot. Benchmark CPU
   only on `--release` (see [LNR-0002](.context/kb/lnr/0002-debug-vs-release-cpu.md)).

## Repository layout

```
src/
  main.rs              thin entry → clipcat::platform::run()
  lib.rs               module wiring
  pet.rs               Pet: platform-agnostic simulation, fish animation,
                       clip/panel orchestration + scene building
  clipboard.rs         ClipStore: clip history model + clips.json persistence
  panel.rs             clipboard panel UI state, layout geometry, hit testing
  render.rs            all vector art (cat, fish, panel, accessories, bubble, icon)
  font.rs              built-in 5×7 pixel font (full printable ASCII)
  hangul.rs            algorithmic vector Hangul (jamo composition, no font files)
  sysfont.rs           system-font text for the panel/toast (ab_glyph, ADR-0007);
                       per-char fallback to font/hangul; the tooltip stays pixel
  hotkey.rs            panel-hotkey spec parsing ("win+shift+v") + display label
  i18n.rs              every user-visible string, English + Korean
  sound.rs             synthesized SFX; winmm on Windows, no-op elsewhere
  state.rs             Persist (JSON) + XP/level progression + accessory table
  input.rs             shared atomic activity counters (KEYS/CLICKS/WHEEL)
  platform/
    mod.rs             selects exactly one backend by cfg
    windows.rs         native Win32 layered window + LL hooks + clipboard
                       listener + global panel hotkey (default Win+Shift+V,
                       fallback Ctrl+Shift+V) + Shell tray
    portable.rs        winit + softbuffer + rdev + arboard (macOS/Linux,
                       and Windows --feature portable)
  bin/gen_icon.rs      regenerates assets/clipcat.ico from render::draw_icon_scaled
examples/preview.rs    renders representative frames to PNGs (headless review)
tests/e2e.rs           end-to-end core flows through the public Pet API
tests/release_script.rs  e2e of scripts/release.sh in a scratch git repo (unix)
build.rs               embeds icon + version info on Windows hosts
scripts/release.sh     the release path: gates, bump, CHANGELOG rotation, tag,
                       push (scripts/release.cmd = Windows wrapper)
CHANGELOG.md           user-facing changes only — see policy in its header
assets/clipcat.ico     the embedded app icon — generated, not hand-edited
assets/screenshot.png  README screenshot (the only committed image asset)
docs/specs/            product & technical specs
.claude/skills/release/  project skill that drives scripts/release.sh
.context/kb/adr/       architecture decision records (why)
.context/kb/lnr/       lessons & near-misses (what bit us)
.github/workflows/     CI: builds on windows + macos + ubuntu (+ changelog lint)
```

## Architecture

Two backends share one core; exactly one backend compiles per build, chosen in
`platform/mod.rs`:

- `all(windows, not(feature = "portable"))` → **native Win32** (default on
  Windows, the release target): per-pixel alpha + click-through layered window,
  `WH_*_LL` hooks, `AddClipboardFormatListener` for copy events (with source
  app name + real icon extraction), a global panel hotkey (default
  Win+Shift+V, configurable via `state.json`, Ctrl+Shift+V fallback on
  clash), Shell tray with full context menu, HKCU autostart.
- `any(not(windows), feature = "portable"))` → **portable**: `winit` window +
  `softbuffer` present + `rdev` global input (counters + the panel-hotkey
  chord matcher; Cmd+Shift+V on macOS) + `arboard` clipboard polling.
  Pet drawn on an opaque card; settings via keyboard shortcuts.

The core flow: a ~33 ms tick drains `input` counters and pending copy events →
`Pet::advance(k,c,wh)` / `Pet::on_copy(text,source,badge)` update animation/
XP/fish/clips → `Pet::render*` builds a `Scene` (plus the panel when open) and
rasterizes with tiny-skia → the backend presents the pixel buffer. The Pet
never touches the OS; the backend never touches simulation internals (only the
public `Pet` API). The only thing a panel interaction asks of the backend is
"put this text on the OS clipboard" (returned as `Option<String>`); backends
suppress the resulting self-triggered clipboard event once. Read
[ADR-0001](.context/kb/adr/0001-cross-platform-architecture.md) and
[ADR-0005](.context/kb/adr/0005-clipboard-manager.md) first.

## Commands

```bash
# Build / run (default backend: native on Windows, portable on macOS/Linux)
cargo build --release
cargo run --release

# Run the PORTABLE backend on Windows (to test the cross-platform path locally)
cargo run --features portable
cargo build --release --features portable

# Quality gates (run all before calling something done)
cargo clippy --release                      # keep clean
cargo clippy --release --features portable  # on Windows, also lint portable path
cargo test --release                        # unit + e2e tests (tests/)

# Visual check without launching the app (works headless)
cargo run --release --example preview       # writes PNGs to /tmp/clipcat-preview

# Regenerate the icon after changing the cat art in render.rs
cargo run --bin gen_icon

# Release (bumps version, rotates CHANGELOG, tags vX.Y.Z, pushes; see the
# `release` project skill). Windows: scripts\release.cmd with the same args.
scripts/release.sh <patch|minor|major> [--dry-run|--no-push]
scripts/release.sh verify                   # CHANGELOG lint (also runs in CI)
```

macOS/Linux need system libs for the portable stack — see the CI workflow's
`Install Linux system dependencies` step for the exact apt list. On a Linux
box without a display, `cargo check --target x86_64-pc-windows-msvc` (and
`--features portable`) cross-checks the Windows code without linking.

## Coding conventions

- Match the surrounding style; module-level `//!` docs explain the "why".
- Core code is `#![forbid]`-clean of OS calls; platform specifics use
  `#[cfg(...)]`, never runtime OS detection.
- Prefer generating assets in code over bundling files.
- Every user-visible string goes through `i18n::t` / an `i18n` helper —
  never hardcode English or Korean in render/backends.
- `unsafe` is confined to `platform/windows.rs` (Win32 FFI) and the small WAV/
  icon byte-buffer builders; document the safety invariant inline.
- Keep both backends' interaction set in parity (drag, single-click bounce,
  double-click pet, hover stats, middle-click/hotkey panel, panel keyboard
  control). If you add an interaction, add it to both.

## Changelog & releases

- `CHANGELOG.md` records **user-facing changes only** (features, behavior
  changes, fixes — written for users). Refactors, CI, docs and other
  dev-environment work stay out; git history covers those. Every PR with a
  user-visible change adds a bullet under `[Unreleased]`.
- Releasing is `scripts/release.sh <bump>` (or the `release` project skill):
  it refuses an empty `[Unreleased]`, runs the quality gates, bumps
  `Cargo.toml`/`Cargo.lock`, rotates `[Unreleased]` into `## [X.Y.Z] - date`,
  commits, tags `vX.Y.Z` (notes in the annotated tag) and pushes.
  `scripts/release.sh verify` lints the changelog format in CI.

## Knowledge base — when to write what

- **ADR** (`.context/kb/adr/NNNN-title.md`): a significant, hard-to-reverse
  decision (a new dependency, a backend, a data-format change). Context →
  Decision → Consequences. Add a row to the ADR index.
- **LNR** (`.context/kb/lnr/NNNN-title.md`): something that cost real
  debugging time and is easy to repeat. Symptom → Cause → Fix → Takeaway.
- **Spec** (`docs/specs/`): product/technical behavior of a feature.

Convert relative dates to absolute when writing these. Link related records with
relative markdown links.

## Verifying a change

1. `cargo build --release`, `cargo clippy --release`, `cargo test --release`
   (plus `--features portable` lint where applicable).
2. `cargo run --release --example preview` and actually look at the PNGs
   (cat, fish, panel in both languages, Hangul sample, icon).
3. On a Windows dev machine: launch the exe, copy text from a couple of apps,
   confirm the fish + history; screenshot the result. Confirm CPU is a few
   percent and memory ~12–16 MB on release.
4. Whatever you could not execute locally (e.g. macOS/Linux runtime from a
   Windows box, or any GUI from a headless box) is validated by CI builds +
   code review — say so honestly in summaries.

## Gotchas (see LNR for detail)

- softbuffer can't do per-pixel desktop alpha → portable uses an opaque card.
- Debug builds render ~10× slower; never judge CPU off a debug run.
- `rdev::listen` blocks → it runs on its own thread; macOS needs Accessibility
  permission, Wayland blocks global capture (and `arboard` without its wayland
  feature needs XWayland for the clipboard).
- The dual optional/`[target.'cfg(...)']` dependency layout is deliberate;
  don't "simplify" it without re-reading
  [LNR-0004](.context/kb/lnr/0004-cargo-cross-platform-deps.md).
- The clipboard watcher must skip the app's own copy-backs exactly once
  (suppression marker), or every panel click would spawn a fish.
- The native window is `WS_EX_NOACTIVATE`; the panel temporarily removes that
  style to take keyboard focus for search, and restores it on close. Don't
  make the plain pet focusable.
