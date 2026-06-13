# ClipCat Dark Premium — Design Package (reference)

This directory is the **ClipCat Dark Premium Desktop App UI** design package,
imported verbatim as an implementation *reference*. It is a design contract, not
runtime code: the SVG/PNG assets here are visual references — ClipCat generates
all runtime art from code (`src/render.rs`) and never bundles assets (see
[ADR-0000](../../.context/kb/adr/0000-zero-framework-rendering.md),
[ADR-0007](../../.context/kb/adr/0007-system-font-ui-text.md),
[ADR-0011](../../.context/kb/adr/0011-remove-pixel-font.md)).

## What's here

| Path | Purpose |
| --- | --- |
| `README.md` | Package overview (goals, scope, priorities) |
| `docs/01_product_contract.md` | Product definition, UX principles, non-negotiables |
| `docs/02_visual_system.md` | Dark-premium visual system, geometry, surfaces, type |
| `docs/03_pet_behavior_spec.md` | Pet state machine, mood transitions, fish/nom, petting, drag, sleep |
| `docs/04_panel_ui_spec.md` | Panel structure, sort/filter, keyboard/mouse, delete safety, empty states |
| `docs/05_error_exception_ux.md` | Suppression, oversized text, storage failure, hotkey clash, permissions, IME |
| `docs/06_rust_architecture.md` | Suggested module map, render order, state structs, dep policy |
| `docs/07_component_inventory.md` | Pet/panel component list + per-component states |
| `docs/08_agent_workflow.md` | Implementation order + definition of done |
| `docs/09_visual_regression_frames.md` | The 12 preview frames to export + pass criteria |
| `tokens/*.json` | Color, radius/spacing, typography design tokens |
| `motion/pet_motion_spec.json` | Per-state timing/easing numbers (tick 33 ms) |
| `handoff/IMPLEMENTATION_MILESTONES.md` | M1–M10 milestone breakdown |
| `qa/QA_CHECKLIST.md` | Privacy/clipboard/panel/pet/visual/platform QA gates |
| `prompts/` | Master prompts for Claude Code / Codex |
| `assets/svg`, `assets/png_preview` | Reference art (NOT runtime dependencies) |

## How the package maps onto the *actual* repo

The package proposes a nested module layout (`src/pet/mod.rs`, `src/panel/layout.rs`,
`src/render/tokens.rs`, …). The repo today uses a **flat** layout, and that is the
convention to keep — treat the package's tree as a logical grouping, not a refactor mandate.

| Package module (suggested) | Lands in (actual repo) |
| --- | --- |
| `pet/state_machine.rs`, `pet/motion.rs`, `pet/xp.rs` | `src/pet.rs` (+ progression in `src/state.rs`) |
| `pet/accessories.rs` | accessory table in `src/state.rs`, drawing in `src/render.rs` |
| `panel/{layout,model,keyboard,search,undo}.rs` | `src/panel.rs` (+ store in `src/clipboard.rs`) |
| `render/{primitives,pet_draw,panel_draw,fx_draw}.rs` | `src/render.rs` |
| `render/tokens.rs` | new constants in `src/render.rs` sourced from `tokens/*.json` |
| `clipboard/{store,watcher}.rs` | `src/clipboard.rs` + the per-backend watchers in `src/platform/` |
| `platform/{windows,macos}.rs` | `src/platform/windows.rs`, `src/platform/portable.rs` + `mac_*.rs` leaves |
| `i18n.rs`, `state.rs` | `src/i18n.rs`, `src/state.rs` (already present) |

## Reconciliation notes (package vs. repo reality)

- **Already satisfied by the repo:** privacy model (counters-only input in
  `src/input.rs`), local-only clipboard store, OS system fonts (`src/sysfont.rs`),
  single sanctioned network exception (`src/update.rs`), movable/resizable panel
  with cat-anchor stability via `take_window_shift`
  ([ADR-0010](../../.context/kb/adr/0010-movable-resizable-panel.md)),
  Win+Shift+V hotkey with Ctrl+Shift+V fallback, EN/KO i18n, Ctrl+0–9 quick copy.
- **Rendering engine:** the package's dep policy lists winit/softbuffer/rdev/arboard/
  ab_glyph/serde — all already present. `tiny-skia` is the rasterizer (the package
  mentions resvg/usvg/tiny-skia only "if code-generated drawing is not enough" — it
  already is; **do not** add resvg/usvg).
- **Mainly a visual + behavior refresh:** the bulk of new work is the *dark-premium
  restyle* (tokens, surfaces, the redesigned cat/fish/panel art), the richer pet
  **mood state machine** (Curious/Sleeping/TypingExtreme/LevelUp, motion numbers in
  `motion/pet_motion_spec.json`), and the **undo stack** + delete/clear-confirm safety
  in the panel — none of which should weaken the existing privacy/parity guarantees in
  `AGENTS.md`.
- **Keep both backends at interaction parity** (`AGENTS.md` "Coding conventions"):
  every panel/pet interaction the package adds must land in both `windows.rs` and
  `portable.rs`.

## Suggested implementation order

Follow `handoff/IMPLEMENTATION_MILESTONES.md` (M1 tokens/primitives → M10 preview+QA),
landing one milestone per change and running `cargo fmt`/`clippy`/`test` +
`cargo run --release --example preview` each time, per `docs/08_agent_workflow.md`
and the repo's own "Verifying a change" checklist in `AGENTS.md`. The 12 required
preview frames are listed in `docs/09_visual_regression_frames.md`.
