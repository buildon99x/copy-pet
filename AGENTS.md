# AGENTS.md — ClipCat contributor & agent guide

ClipCat is a tiny, dependency-light **clipboard-manager desktop pet** written
in Rust. A cat sits at the bottom of the screen and taps along with your
keyboard/mouse; every system-wide text copy is captured into a local, searchable
clipboard history — visualized as the cat eating a fish badged with the source
app. Copying, typing and clicking earn XP, level the cat up and unlock
accessories. The full UI is bilingual (English/Korean). Primary target is
**Windows** (premium native build); **macOS** and **Linux** are supported via a
portable backend.

> This file is the source of truth for *how to work* in this repo (the steering
> that's worth loading every turn); it links out to reference detail — the repo
> map, ADRs, LNRs — that you read only when a task touches it. `CLAUDE.md`
> imports this file. Keep it accurate when structure or commands change.

## Golden rules

1. **Privacy is non-negotiable.** Global *input* hooks may only increment the
   atomic counters in `src/input.rs`, with exactly two sanctioned exceptions:
   (a) the portable backend's `ChordTracker` compares each key event against
   the user's configured panel-hotkey chord and immediately discards it
   (ADR-0008), and (b) the auto-repeat gate (`input::key_down/key_up` on
   Windows, the portable `KeyGate`) reduces each keycode to a held/released
   bit so holding a key counts once, dropping it on release — beyond that,
   never read, store, log or transmit key contents, window titles or
   timings. Clipboard *content* is the product,
   but it stays local: stored only in the user's config dir, and capture can
   always be paused. The **single sanctioned network exception** is the
   update check/download in `src/update.rs` (ADR-0009): it talks to
   github.com releases via the system `curl`, transmits nothing beyond the
   request itself, and is switchable off (`auto_update`). **Add no other
   network use.**
2. **Keep the core platform-agnostic.** Simulation, clipboard store, panel
   logic, rendering, i18n, progression and persistence live in the core and
   must not reference any OS API (tiny per-OS leaves like `state::today_string`
   / `state::detect_lang` and the `cfg` leaves in `update.rs` are the only
   sanctioned exceptions). OS code lives only under `src/platform/`.
3. **No new heavy dependencies** without an ADR. The whole point is a small
   binary with a handful of crates and no asset pipeline (icon and sounds
   are generated from code; all text uses fonts read from the OS at
   runtime — never bundled, see ADR-0007/ADR-0011).
4. **Verify by rendering/running, on release.** Build, run the tests, and
   eyeball `cargo run --release --example preview` PNGs (headless-friendly).
   On a Windows dev machine, also launch the exe and screenshot. Benchmark CPU
   only on `--release` (see [LNR-0002](.context/kb/lnr/0002-debug-vs-release-cpu.md)).

## Repository layout

One platform-agnostic core (`src/*.rs`, no OS calls) plus exactly one compiled-in
backend under `src/platform/` (native Win32, or portable winit/softbuffer/rdev/
arboard). The full file-by-file map lives in
[`.context/repo-map.md`](.context/repo-map.md) — reference, read on demand (kept
out of this always-loaded file on purpose).

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
  `softbuffer` present + global input (counters + the panel-hotkey chord
  matcher; Cmd+Shift+V on macOS) + `arboard` clipboard polling. Global input
  is `rdev::listen` on Linux/Windows, but a bespoke CoreGraphics event tap
  (`platform/mac_input.rs`) on macOS — rdev's keyboard path crashes on macOS
  15 (LNR-0005). Settings via keyboard shortcuts. Presentation: an opaque
  "card" via softbuffer on Linux/Windows-portable, but a **transparent**
  CALayer present on macOS (`platform/mac_present.rs`) so the pet floats like
  the Windows layered window (ADR-0003 update).

The core flow: a ~33 ms tick drains `input` counters and pending copy events →
`Pet::advance(k,c,wh)` / `Pet::on_copy(text,source,badge)` update animation/
XP/fish/clips → `Pet::render*` builds a `Scene` (plus the panel when open) and
rasterizes with tiny-skia → the backend presents the pixel buffer. The Pet
never touches the OS; the backend never touches simulation internals (only the
public `Pet` API). The only thing a panel interaction asks of the backend is
"put this text on the OS clipboard" (returned as `Option<String>`); backends
suppress the resulting self-triggered clipboard event once. Window geometry
is also a Pet contract: when `take_size_changed()` fires, the backend resizes
to `canvas_size()` **and** offsets the window by `take_window_shift()` — that
shift keeps the cat anchored on screen while the panel opens, moves, resizes
or the scale changes (panel-card drags flow through
`Pet::panel_drag_start/_update/_end` with screen-pixel deltas / scale). Read
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

macOS/Linux need system libs for the portable stack — on Debian/Ubuntu:
`apt-get install libx11-dev libxi-dev libxtst-dev libxkbcommon-dev
libxkbcommon-x11-dev pkg-config` (CI has no Linux job to copy this from).
On a Linux
box without a display, `cargo check --target x86_64-pc-windows-msvc` (and
`--features portable`) cross-checks the Windows code without linking;
`cargo check/clippy --target aarch64-apple-darwin` does the same for the macOS
code (e.g. `platform/mac_input.rs`) — both type-check the `#[cfg]`-gated paths
a Linux build can't reach.

## Coding conventions

- Match the surrounding style; module-level `//!` docs explain the "why".
- Core code is `#![forbid]`-clean of OS calls; platform specifics use
  `#[cfg(...)]`, never runtime OS detection.
- Prefer generating assets in code over bundling files.
- Every user-visible string goes through `i18n::t` / an `i18n` helper —
  never hardcode English or Korean in render/backends.
- `unsafe` is confined to the OS-FFI leaves — `platform/windows.rs` (Win32) and
  the macOS `mac_input` / `mac_present` / `mac_menu` / `mac_dialogs` files
  (CoreGraphics / Objective-C) — plus the small WAV/icon byte-buffer builders;
  document the safety invariant inline. (`mac_autostart.rs` is plain `std::fs`.)
- Keep both backends' interaction set in parity (drag, single-click bounce,
  double-click pet, hover stats, middle-click/hotkey panel, panel keyboard
  control incl. Ctrl+0-9 quick copy, panel header-drag move + grip-drag
  resize). If you add an interaction, add it to both. The settings menu is the
  one deliberate split: Windows native uses the Shell tray menu, macOS renders
  the shared `menu::MenuEntry` model (`Pet::build_menu`) as a right-click NSMenu
  (`platform/mac_menu.rs`) at full tray parity; Linux/Windows-portable still
  rely on the keyboard shortcuts (no native menu wired there yet). Menu *items
  and their effects* live in `menu.rs` + `Pet::{build_menu,apply_menu_action}`
  (testable); add new menu actions there, not in a backend.

## Changelog & releases

- `CHANGELOG.md` records **user-facing changes only** (features, behavior
  changes, fixes — written for users). Refactors, CI, docs and other
  dev-environment work stay out; git history covers those. Every PR with a
  user-visible change adds a bullet under `[Unreleased]`.
- Cutting a release is `scripts/release.sh <bump>` (or the `release` skill — see
  Commands): it refuses an empty `[Unreleased]`, runs the gates, bumps the
  version, rotates `[Unreleased]` into `## [X.Y.Z] - date`, tags `vX.Y.Z` (notes
  in the annotated tag) and pushes. `scripts/release.sh verify` lints it in CI.

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

1. Run the quality gates (see Commands: `build`, `clippy` ± `portable`, `test`).
2. `cargo run --release --example preview` and actually look at the PNGs
   (cat, fish, panel in both languages, Hangul sample, icon).
3. On a Windows dev machine: launch the exe, copy text from a couple of apps,
   confirm the fish + history; screenshot. CPU a few percent, memory ~12–16 MB
   on release.
4. If you touched the clip-pick / paste path (`Pet::run_action`, `ClipPick`,
   `copy_back` / `set_clipboard`, or any panel setting): on Windows exercise
   **both** trigger paths — the hotkey **flyout** (focus a text field, press the
   hotkey, pick a clip → it must paste into that field, Win+V parity) **and** the
   middle-click panel — and confirm any new setting is actually reachable on the
   **Windows tray menu**, not only the macOS NSMenu (the menu is the deliberate
   backend split; a shared `menu::MenuAction` is easy to ship with no Windows
   surface — see [LNR-0007](.context/kb/lnr/0007-flyout-paste-gated-on-unreachable-setting.md)).
5. Whatever you could not execute locally (e.g. macOS/Linux runtime from a
   Windows box, or any GUI from a headless box) is validated by CI builds +
   code review — say so honestly in summaries. Note CI has **no Linux job**;
   Linux-affecting changes need a local Linux build/test pass.

## Gotchas

Before touching rendering, global input, the cross-platform dependency layout or
the macOS path, read the [LNR index](.context/kb/lnr/README.md) first — the
recurring traps (softbuffer's lack of per-pixel alpha, debug-vs-release CPU,
`rdev::listen` threading + macOS Accessibility/Wayland, the macOS
TIS-off-the-main-thread SIGTRAP crash, the deliberate optional/`cfg` dep layout)
each have a record. Three live invariants with no LNR home:

- The clipboard watcher must skip the app's own copy-backs exactly once
  (suppression marker), or every panel click would spawn a fish.
- The native window is `WS_EX_NOACTIVATE`; the panel temporarily removes that
  style to take keyboard focus for search, and restores it on close. Don't
  make the plain pet focusable.
- The release asset names in `ci.yml` and `update::WINDOWS_ASSET` are a
  contract — the updater builds its download URLs from them (and from
  `Cargo.toml`'s `repository`). Change one, change all (ADR-0009).
