# Plan: Add the Pudding Accessory

**Task:** #44 · Add the pudding accessory
**Date:** 2026-06-19
**Branch:** `claude/add-the-pudding-accessory-20260619`

---

## Context

ClipCat has 6 unlockable accessories (Scarf→Glasses→Beanie→Headphones→Crown→Wizard, ids 1–6).
A pudding accessory (Japanese purin-style flan: custard trapezoid + caramel glaze + drip + cherry) needs to be wired into the shipped lineup as id 7.

There is no prior Pudding code in the repo; both the render arm and the ACCESSORIES row need to be added from scratch. The issue description stated the draw code "already exists" but inspection of the current branch shows neither the enum variant nor the draw arm is present.

---

## Approach

Three targeted edits, no new files beyond the plan doc:

### 1. `src/render.rs`

**a) Enum** — add `Pudding` after `Wizard` in the `Accessory` enum.

**b) `from_id`** — add `7 => Accessory::Pudding` (keeps the 1-N contiguous mapping).

**c) `draw_accessory` match arm** — draw a Japanese-style pudding hat sitting on the cat's head:
- Custard body: filled trapezoid (wide base ≈ y 98, narrow top ≈ y 60, centered x 120) in custard yellow.
- Base disc: flat oval underscoring the trapezoid.
- Caramel glaze: darker amber oval resting on the top surface.
- Caramel drip: short open path curling down the right side.
- Cherry + stem: red oval + thin green curve at the apex.

### 2. `src/state.rs`

- Change array type to `[AccessoryDef; 7]`.
- Append `AccessoryDef { level: 20, name_kr: "푸딩", name_en: "PUDDING" }`.

Level 20 is the next natural capstone above Wizard Hat (Lv 15) and safely within the 2–99 range validated by the existing test.

### 3. `CHANGELOG.md`

Add one `Added` bullet under `[Unreleased]`.

---

## Files to change

| File | Change |
|---|---|
| `src/render.rs` | +`Pudding` variant, +`from_id` arm, +`draw_accessory` arm |
| `src/state.rs` | Array size 6→7, new ACCESSORIES row |
| `CHANGELOG.md` | `[Unreleased] Added` bullet |

---

## Verification

```
cargo build --release
cargo clippy --release
cargo test --release         # includes every_accessory_has_a_reachable_level
cargo run --release --example preview   # visually confirm pudding in collection grid
```

No i18n changes needed (AccessoryDef keeps `name_kr`/`name_en` fields unchanged; i18n migration is tracked in #45).
No platform-specific changes (menu code iterates ACCESSORIES generically).
