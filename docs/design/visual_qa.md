# ClipCat Pet & Motion Visual QA

## Must Match Reference

- Cat is white/cream with soft rounded forms.
- Red scarf is visible and consistent.
- Idle cat faces forward.
- Typing states show keyboard and active paws.
- Extreme typing has high-energy FX.
- Sleep state is curled with blue Zzz.
- Copy event has fish, badge, flight path, open mouth, nom, happy, XP popup.
- Hover bubble has level, XP bar, today stats, and cat peek.
- Accessories include scarf, glasses, beanie, headphones, crown, wizard hat.
- Panel preview opens from cat area without moving the cat.

## Failure Conditions

Fail if:

- Cat changes species or silhouette between states.
- Fish does not end at mouth target.
- Mouth opens too late or not at all.
- XP popup appears before nom.
- Hover bubble covers pet completely.
- Panel open shifts cat anchor by more than 1 px.
- Motion jitters at 60 FPS.
- Animation speed differs by more than 15 percent from spec.
- Korean labels render as tofu boxes.
- Missing assets cause app crash.

## Required Preview Frames

Export these frames for review:

```txt
preview_idle.png
preview_typing_slow.png
preview_typing_fast.png
preview_typing_extreme.png
preview_sleep.png
preview_yawn.png
preview_look_around.png
preview_copy_sequence_01_spawn.png
preview_copy_sequence_02_badge.png
preview_copy_sequence_03_flight.png
preview_copy_sequence_04_mouth_open.png
preview_copy_sequence_05_nom.png
preview_copy_sequence_06_happy_xp.png
preview_hover_bubble.png
preview_accessories.png
preview_panel_open.png
```
