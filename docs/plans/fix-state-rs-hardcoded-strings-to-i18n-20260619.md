# Plan: Port state.rs hardcoded accessory names to i18n

**Issue:** #45  
**Branch:** `claude/fix-state-rs-hardcoded-strings-to-i18n-20260619`  
**Date:** 2026-06-19

## Context

`src/state.rs` holds an `ACCESSORIES` constant array of `AccessoryDef` structs, each with
`name_kr: &'static str` and `name_en: &'static str` fields. This violates the project rule
(AGENTS.md): *"Every user-visible string goes through `i18n::t` / an `i18n` helper — never
hardcode English or Korean in render/backends."*

The issue body mentions 6 named variants but the scope must cover **all 19** accessories — you
cannot replace `name_kr`/`name_en` with a single `msg: Msg` field unless every entry has a
corresponding `Msg` variant. The acceptance criteria (criteria 2 & 3) confirm full replacement.

## Approach

### 1. `src/i18n.rs` — add 19 `Msg` variants

Add these to the `Msg` enum (after existing `AccNone`):

```
AccRedScarf, AccGlasses, AccBlueBeanie, AccHeadphones, AccGoldCrown, AccWizardHat,
AccBunnyEars, AccSprout, AccDaisyCrown, AccBearEars, AccCherry, AccButterfly,
AccHeartShades, AccChick, AccSleepMask, AccNightcap, AccFishHat, AccFishBread, AccLuckyClover,
```

Add their EN/KO translations to `t()` — re-using the exact strings already in `state.rs` so
no visible change to users.

Update the `every_message_has_both_translations` test to include all 19 new variants in the
`all` array.

### 2. `src/state.rs` — replace struct fields with `msg: Msg`

- Change `AccessoryDef` to:
  ```rust
  pub struct AccessoryDef {
      pub level: u32,
      pub msg:   i18n::Msg,
  }
  ```
- Change `name()`:
  ```rust
  pub fn name(&self, lang: Lang) -> &'static str {
      i18n::t(lang, self.msg)
  }
  ```
- Update all 19 rows of `ACCESSORIES` to use `msg: i18n::Msg::AccXxx` instead of `name_kr`/`name_en`.
- Update the `every_accessory_has_a_reachable_level` test: replace the `name_en`/`name_kr`
  emptiness assertions with `!t(Lang::En, acc.msg).is_empty()` and `!t(Lang::Ko, acc.msg).is_empty()`.

### 3. Call sites — no change needed

`acc.name(lang)` in `src/pet.rs` and `src/platform/windows.rs` already call the public
`name()` method; the signature stays the same so no edits are required there.

## Files to change

| File | Change |
|------|--------|
| `src/i18n.rs` | Add 19 `Msg` variants + translations; update test |
| `src/state.rs` | Replace `AccessoryDef` fields; update `ACCESSORIES` array; update test |

## Verification

```
cargo build --release
cargo clippy --release
cargo test --release
```

All should be green. The `every_accessory_has_a_reachable_level` (state) and
`every_message_has_both_translations` (i18n) tests directly cover the change.
