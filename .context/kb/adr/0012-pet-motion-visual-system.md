# ADR-0012: Pet motion & visual system — code-generated soft-shaded restyle

- Status: Accepted
- Date: 2026-06-13
- Related: [ADR-0000](0000-zero-framework-rendering.md) (tiny-skia vector art),
  [ADR-0007](0007-system-font-ui-text.md) / [ADR-0011](0011-remove-pixel-font.md)
  (generate assets in code, no bundled art), [ADR-0003](0003-portable-rendering-card.md)
  (opaque portable card), [ADR-0010](0010-movable-resizable-panel.md) (panel canvas + cat anchor)

## Context

A "Pet Motion Design Contract" ([`docs/design/`](../../docs/design)) was supplied
to lift ClipCat's pet from a flat vector cat to a premium, soft-shaded "3D-like"
creature with an explicit state machine (idle, three typing tiers, sleep, yawn,
look-around, the copy→nom→XP sequence, petting, boop, level-up, new-item, panel
open) and a dark-glass hover stats card. The contract's `asset_manifest.json`
*prefers* "layered PNG or SVG parts," and its `codex_prompt.md` proposes a new
`src/pet/` submodule tree.

The repo already implements ~85% of the contract's *mechanics* (fish queue +
bezier flight + nom + happy, the stats bubble, petting/boop/copy XP, level-up +
accessory unlocks with the exact Lv 2/3/5/7/10/15 table, the cat-anchored panel
window-shift, delta-time animation, EN/KO i18n). The gaps are *visual* (flat
fills + a hard brown outline; a white bubble) plus a few behaviors (discrete
typing tiers, yawn/look-around, a "+N XP" popup, a panel open/close animation).

Two of the contract's suggestions collide with the repo's golden rules:
bundling sprite art violates rule #3 + ADR-0007/0011 (generate in code, no asset
pipeline, tiny binary), and the `src/pet/` explosion contradicts the flat,
small-module convention.

## Decision

- **Restyle in code, not with bundled assets.** Keep the tiny-skia vector art in
  `render.rs` and elevate it: per-part **gradient shading** (a soft "ball"
  radial/linear shade for head/body/paws), a stacked-oval soft drop shadow, a
  pink nose, the contract palette from `pet_tokens.json` (`furBase #FFF4EA`,
  `furShadow #E8D1C1`, `scarfRed #B7352E`, `sparkle #FFC928`, …), and **no hard
  outline** (token `outline.enabled=false`, soft edges). The hover bubble becomes
  a **dark-glass card** with a yellow accent and a peeking cat.
  - **Visual tone = "Matte soft":** flat, gentle fur-shadow gradient with **no
    specular highlight** (high ambient term, zero gloss) — chosen from a 4-up
    tone study. Calm/premium rather than glossy.
  - This keeps the binary tiny, screenshot tests deterministic, accessory
    composition + high-DPI scaling trivial, and honors ADR-0007/0011. The cost is
    that we *approximate* a rendered illustration rather than matching one
    pixel-for-pixel — an accepted trade for the no-asset-pipeline guarantee.
- **Extend the existing flat modules; do not create `src/pet/`.** New state lives
  as focused fields/functions on `Pet` (`pet.rs`) and `Scene` (`render.rs`):
  a discrete `typing_tier` derived from the existing smoothed `rate`; `yawn` /
  `look` idle-gesture envelopes with a random scheduler; an `xp_popup` floating
  text started inside `nom()` (so it never precedes the nom); and a
  `panel_phase`/`panel_anim` that scales the card on open/close while the cat
  stays anchored (the window grows up front on open and only shrinks after the
  close animation finishes).
- **No change to the golden rules.** All of this is platform-agnostic core; no
  new dependencies, no network, no input-content reads. The contract files are
  committed under `docs/design/` (text) + `assets/reference/` (one board PNG) as
  a living spec, with implementation status tracked in
  [`docs/specs/pet-motion.md`](../../docs/specs/pet-motion.md).

## Consequences

- ✅ A markedly richer, premium-looking pet with no asset pipeline, no binary
  bloat (one 2.5 MB *doc* PNG, never shipped), and unchanged determinism for the
  `examples/preview` QA frames + e2e tests.
- ✅ The contract is now in-repo and versioned; future motion work has a single
  source of truth and a status tracker.
- ⚠️ The visual ceiling is lower than hand-drawn/3D-rendered sprites; if a future
  product bar demands photoreal fidelity, *that* is the decision that would
  require a new ADR (bundled assets + an asset pipeline + a deterministic-render
  story for tests), superseding this one.
- ⚠️ Gradients per part add a little per-frame fill cost vs. flat fills; it stays
  well within the "few percent CPU on release" budget (benchmark on `--release`,
  [LNR-0002](../lnr/0002-debug-vs-release-cpu.md)) and the asleep-frame skip is
  unchanged.
- ⚠️ The panel close now animates before the window shrinks, so the window-resize
  timing differs from the old instant close; the cat-anchor contract
  (`take_window_shift`) is preserved and the affected e2e tests advance ticks to
  let the animation settle.
