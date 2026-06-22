# Removed accessories — historical record

These accessories were part of the ClipCat lineup before the 2026-06-22 renewal (issue #55).
Their code (enum variants, draw functions, i18n names, unlock entries) was removed entirely;
this document is the sole record.

## Previously active (were in `state::ACCESSORIES`)

| Old id | render variant | i18n Msg | English label | Korean label | Old unlock level |
|--------|----------------|----------|---------------|--------------|-----------------|
| 7 | `BunnyEars` | `AccBunnyEars` | BUNNY EARS | 토끼 귀 | 18 |
| 9 | `FlowerCrown` | `AccDaisyCrown` | DAISY CROWN | 데이지 화관 | 24 |
| 10 | `BearEars` | `AccBearEars` | BEAR EARS | 곰 귀 | 27 |
| 11 | `Cherry` | `AccCherry` | CHERRY | 체리 | 30 |
| 12 | `Butterfly` | `AccButterfly` | BUTTERFLY | 나비 | 33 |
| 13 | `HeartGlasses` | `AccHeartShades` | HEART SHADES | 하트 선글라스 | 36 |
| 14 | `Chick` | `AccChick` | CHICK | 병아리 | 39 |
| 15 | `SleepMask` | `AccSleepMask` | SLEEP MASK | 수면 안대 | 42 |
| 16 | `Nightcap` | `AccNightcap` | NIGHTCAP | 수면 모자 | 45 |
| 17 | `FishHat` | `AccFishHat` | FISH HAT | 생선 모자 | 48 |
| 18 | `Bungeoppang` | `AccFishBread` | FISH BREAD | 붕어빵 | 50 |

## Previously "future drop" (render code only — never in `state::ACCESSORIES`)

These had drawn artwork in `render.rs` but were not yet offered in any menu.

| Old id | render variant | Notes |
|--------|----------------|-------|
| 21 | `Ribbon` | Soft bow between the ears |
| 22 | `Flower` | Small flower by the right ear |
| 23 | `Beret` | Flat beret on the crown |
| 24 | `Hood` | Cozy raised hood |
| 25 | `BlanketCape` | Blanket draped over the shoulders |
| 26 | `StarPin` | Gold star hairpin by the right ear |
| 27 | `Halo` | Golden halo floating above the head |
| 28 | `Strawberry` | Strawberry beanie |
| 29 | `MoonStar` | Crescent moon and star above the head |
| 30 | `Cloud` | Fluffy cloud above the head |
