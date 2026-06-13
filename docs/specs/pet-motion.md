# ClipCat — Pet Motion & Visual System

Status: implemented (first pass; panel open/close animation deferred) · Owner: ClipCat ·
Last updated: 2026-06-13 ·
Contract: [`docs/design/`](../design) · Decision: [ADR-0012](../../.context/kb/adr/0012-pet-motion-visual-system.md)

## 1. Summary

This spec tracks the pet appearance + motion upgrade driven by the design
contract in [`docs/design/`](../design) (state machine, motion clips, tokens,
QA). The cat is rendered **entirely in code** (tiny-skia vector art in
`render.rs`), not from bundled sprites — see ADR-0012. Visual tone is **"Matte
soft"**: flat, gentle fur-shadow shading with no specular highlight, deep-red
scarf, no hard outline.

The contract's *mechanics* were already ~85% implemented; this work closes the
visual gap and the missing behaviors. Source of truth for behavior is the
contract YAML/JSON; this file records **what is done vs. pending**.

## 2. Implementation status

Legend: ✅ done · ⬜ deferred

### Visuals
| Item | Status | Where |
|------|:--:|------|
| Soft-shaded fur (matte vertical gradients head/body/ears/paws/tail) | ✅ | `render.rs` (`vgrad`/`fill_grad_t`) |
| Contract palette (`pet_tokens.json`) | ✅ | `render.rs` |
| Remove hard outline / soft edges | ✅ | `render.rs` |
| Soft stacked-oval drop shadow + pink nose + tongue | ✅ | `render.rs::draw_face` |
| Dark-glass hover stats card (yellow accent, cat peek) | ✅ | `render.rs::draw_bubble` |
| Accessories (scarf re-shaded; glasses/beanie/headphones/crown/wizard) | ✅ | `render.rs::draw_accessory` |

### States / behaviors
| Item | Status | Where |
|------|:--:|------|
| Idle (blink, breath, tail) | ✅ | `pet.rs` / `render.rs` |
| Sleep + Zzz after idle | ✅ | `pet.rs` |
| Discrete typing tiers (slow / fast / extreme + extreme energy FX) | ✅ | `pet.rs` (`tier_for_rate`) |
| Yawn + look-around idle gestures | ✅ | `pet.rs` (`Gesture`/`gesture_envelope`) |
| Copy → fish + badge + fly-to-mouth + nom + happy | ✅ | `pet.rs` |
| "+N XP" floating popup (starts at nom, never before) | ✅ | `pet.rs::start_xp_popup` / `render.rs` |
| Petting (double-click, +10 XP, hearts) | ✅ | `pet.rs::pet` |
| Boop (single-click, +1 XP) | ✅ | `pet.rs::click_bounce` |
| Level-up (stars + "LEVEL UP!") | ✅ | `pet.rs::maybe_level_up` |
| New-accessory unlock + auto-equip | ✅ | `pet.rs` |
| Panel open/close grow animation (cat anchored) | ⬜ | deferred — see §3 |

### QA
| Item | Status | Where |
|------|:--:|------|
| Preview frames (states board, accessories board, +XP, fish, bubble EN/KO) | ✅ | `examples/preview.rs` |
| e2e/unit coverage for new behaviors | ✅ | `pet.rs` tests (tiers, gesture, +XP ordering) |

## 3. Deferred

- ⬜ **Panel open/close grow animation** (`motion_spec.yaml: panel_open/close`).
  A correct animated *close* requires deferring the window shrink until the
  card finishes collapsing, which changes the timing of the cat-anchor
  contract (`take_window_shift`) and the public `panel_open()` semantics that
  both backends and the e2e suite depend on. That anchor behavior is exactly the
  `visual_qa.md` fail condition ("panel open shifts cat anchor by > 1px") and
  can only be validated on a real Windows/macOS window — not headlessly. It is
  intentionally left for a pass that can be GUI-verified; the panel still opens
  instantly and keeps the cat anchored as before.
- ⬜ Bundled brand-specific source badges (VS Code/Chrome/Notion/…). We keep the
  current privacy- and asset-friendly approach: a stable hashed-colour initial
  chip, plus the real exe icon on Windows.
- ⬜ A full `new_item` reveal *card* (today: a toast + auto-equip).
- ⬜ Photoreal/illustrated sprite art (rejected by ADR-0012; would need an asset
  pipeline and a deterministic-render story for tests).

## 4. Notes

- Privacy/golden rules unchanged: all core, no new deps, no network, no
  input-content reads. Typing tiers read only the existing activity counters.
- Determinism: `render` stays free of randomness so the `examples/preview` QA
  frames and screenshot tests are stable.
