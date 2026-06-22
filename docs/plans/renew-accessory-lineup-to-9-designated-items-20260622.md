# Plan: Renew accessory lineup to 9 designated items

**Issue:** #55  
**Date:** 2026-06-22  
**Branch:** claude/wizardly-goodall-urbfdi

---

## Context

The codebase currently has 20 accessories in `state::ACCESSORIES` (ids 1–20) plus 10 additional
"future drop" variants in `render::Accessory` (ids 21–30) that are drawn but not offered in any
menu. The task is to keep only the 9 designated accessories and **strip all other code entirely**
(enum variants, draw functions, id mappings, i18n names, unlock entries). A `docs/` record captures
what was removed.

### Current state

| Source | Count |
|--------|-------|
| `render::Accessory` variants (excl. `None`) | 30 |
| `state::ACCESSORIES` entries | 20 |
| `i18n::Msg` Acc* variants | 20 |

### Accessories to **keep** (9)

| new id | i18n Msg | render variant | EN | KO | level |
|--------|----------|----------------|----|----|-------|
| 1 | AccRedScarf | Scarf | Red scarf | 빨간 목도리 | 2 |
| 2 | AccGlasses | Glasses | Glasses | 동그란 안경 | 3 |
| 3 | AccBlueBeanie | Beanie | Blue beanie | 파란 비니 | 5 |
| 4 | AccHeadphones | Headphones | Headphones | 헤드폰 | 7 |
| 5 | AccGoldCrown | Crown | Gold crown | 황금 왕관 | 10 |
| 6 | AccWizardHat | Wizard | Wizard hat | 마법사 모자 | 15 |
| 7 | AccSprout | Sprout | Sprout | 새싹 | 21 |
| 8 | AccLuckyClover | Clover | Lucky clover | 네잎클로버 | 54 |
| 9 | AccPudding | Pudding | Pudding | 푸딩 | 57 |

### Accessories to **remove** (all others)

Active (were in `state::ACCESSORIES`): BunnyEars (7), DaisyCrown (9), BearEars (10), Cherry (11),
Butterfly (12), HeartGlasses/HeartShades (13), Chick (14), SleepMask (15), Nightcap (16),
FishHat (17), Bungeoppang/FishBread (18).

Future-drop (were in render only, not in ACCESSORIES): Ribbon (21), Flower (22), Beret (23),
Hood (24), BlanketCape (25), StarPin (26), Halo (27), Strawberry (28), MoonStar (29), Cloud (30).

---

## Approach

Three source files change; one docs file is created.

### 1. `src/render.rs`

**`Accessory` enum** — remove 21 variants, keep `None` + 9:
- Remove: Ribbon, BunnyEars, Flower, Beret, Hood, BlanketCape, SleepMask, FishHat, Nightcap,
  FlowerCrown, StarPin, BearEars, Halo, Bungeoppang, Strawberry, HeartGlasses, MoonStar, Chick,
  Butterfly, Cherry, Cloud.

**`from_id`** — renumber to map 1–9 only:
```
1→Scarf, 2→Glasses, 3→Beanie, 4→Headphones, 5→Crown, 6→Wizard,
7→Sprout, 8→Clover, 9→Pudding, _→None
```
(Old ids 8/Sprout becomes 7, 19/Clover becomes 8, 20/Pudding becomes 9. Saved states with
removed-accessory ids will resolve to `None` — acceptable, no migration code needed.)

**`draw_accessory`** — remove the 21 match arms for the removed variants.

### 2. `src/state.rs`

**`ACCESSORIES`** — trim from `[AccessoryDef; 20]` to `[AccessoryDef; 9]`, keeping only the
9 rows in id order. Unlock levels are unchanged from the originals.

**Test `levels_advance_monotonically`** — the loop range `1..20u32` was coincidentally chosen to
match the old accessory count; bump it to `1..60u32` so it tests enough levels to cover the
highest unlock (level 57) and serves as a general level-system check.

### 3. `src/i18n.rs`

**`Msg` enum** — remove 11 Acc* variants:
`AccBunnyEars, AccDaisyCrown, AccBearEars, AccCherry, AccButterfly, AccHeartShades, AccChick,
AccSleepMask, AccNightcap, AccFishHat, AccFishBread`.

**String table** — remove the 11×2 (EN+KO) string entries for those variants.

### 4. `docs/removed-accessories.md`

New file documenting every removed accessory: render variant name, old id, old unlock level,
EN/KO label — so the record lives outside the code.

---

## Files to change

| File | Change |
|------|--------|
| `src/render.rs` | Remove 21 enum variants + their draw arms; renumber `from_id` |
| `src/state.rs` | Trim ACCESSORIES to 9; expand test loop range |
| `src/i18n.rs` | Remove 11 Msg variants + their string table entries |
| `docs/removed-accessories.md` | New: record of removed items |

---

## Verification

1. `cargo build --release` — no dead-code warnings, no compile errors.
2. `cargo clippy --release` — clean.
3. `cargo test --release` — all tests pass (accessory + level tests updated).
4. `cargo run --release --example preview` — PNGs look correct for EN and KO; the cat renders
   with each of the 9 kept accessories without panic.
