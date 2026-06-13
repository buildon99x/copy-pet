# ClipCat — Pet Motion & Visual System

Status: in progress · Owner: ClipCat · Last updated: 2026-06-13 ·
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

Legend: ✅ done · 🟡 this pass · ⬜ deferred

### Visuals
| Item | Status | Where |
|------|:--:|------|
| Soft-shaded fur (gradient head/body/paws), matte tone | 🟡 | `render.rs` |
| Contract palette (`pet_tokens.json`) | 🟡 | `render.rs` |
| Remove hard outline / soft edges | 🟡 | `render.rs` |
| Soft drop shadow + pink nose | 🟡 | `render.rs` |
| Dark-glass hover stats card (yellow accent, cat peek) | 🟡 | `render.rs::draw_bubble` |
| Accessories (scarf, glasses, beanie, headphones, crown, wizard) | ✅ | `render.rs::draw_accessory` |

### States / behaviors
| Item | Status | Where |
|------|:--:|------|
| Idle (blink, breath, tail) | ✅ | `pet.rs` / `render.rs` |
| Sleep + Zzz after idle | ✅ | `pet.rs` |
| Discrete typing tiers (slow / fast / extreme + extreme FX) | 🟡 | `pet.rs` |
| Yawn + look-around idle gestures | 🟡 | `pet.rs` / `render.rs` |
| Copy → fish + badge + fly-to-mouth + nom + happy | ✅ | `pet.rs` |
| "+N XP" floating popup (after nom) | 🟡 | `pet.rs` / `render.rs` |
| Petting (double-click, +10 XP, hearts) | ✅ | `pet.rs::pet` |
| Boop (single-click, +1 XP) | ✅ | `pet.rs::click_bounce` |
| Level-up (stars + "LEVEL UP!") | ✅ | `pet.rs::maybe_level_up` |
| New-accessory unlock + auto-equip | ✅ | `pet.rs` |
| Panel open/close grow animation (cat anchored) | 🟡 | `pet.rs` / `panel.rs` / `render.rs` |

### QA
| Item | Status | Where |
|------|:--:|------|
| Preview frames per `visual_qa.md` | 🟡 | `examples/preview.rs` |
| e2e/unit coverage for new behaviors | 🟡 | `tests/e2e.rs`, `pet.rs` tests |

## 3. Deferred (would need their own change/ADR)

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
