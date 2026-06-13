# ClipCat Pet & Motion Implementation Brief

## Purpose

This package converts the provided ClipCat pet-motion concept board into an implementation-ready contract for Codex or Claude Code.

The goal is to implement the cat appearance, pet states, copy/fish/nom sequence, hover bubble, accessories, effects, panel-open motion, and interaction feedback so the actual Rust desktop app behaves like the reference board.

## Target

- Product: ClipCat
- Platform: Windows and macOS
- Language: Rust
- Rendering approach: native desktop rendering; prefer the existing project renderer. If absent, use winit + softbuffer + tiny-skia or equivalent immediate rendering.
- Pet style: cute premium 3D-like illustrated cat, white fur, red scarf, soft highlights, dark premium UI panels.
- Motion style: snappy, friendly, low-latency, subtle idle life, expressive copy reaction.

## Output Intent

Codex or Claude Code should use this package to implement:

1. Pet appearance system
2. Pet state machine
3. Animation clip timing
4. Fish + source badge animation
5. Hover stats bubble
6. Petting and boop reactions
7. Accessory unlock/attachment system
8. Panel open/close animation
9. FX sprites/particles
10. Audio hooks
11. E2E behavior and visual QA

## Important Product Principle

ClipCat is not just a clipboard manager with decoration.

ClipCat is a small desktop creature that visually eats copied text and grows through the user’s daily activity.

## Build Order

1. Render static pet states from reference.
2. Implement pet state machine with mock events.
3. Implement idle, sleep, typing, copy, petting, boop, level-up, accessory clips.
4. Add fish queue and copy reaction.
5. Add hover stats bubble.
6. Add panel open/close animation.
7. Connect real clipboard events.
8. Connect XP/level/accessory unlocks.
9. Add audio hooks.
10. Add visual regression frames.

## Non-negotiable Requirements

- Cat must remain visually consistent across states.
- Red scarf appears as default accessory in most states.
- Copy event must show fish flying into cat mouth.
- Source badges must match app identity or fallback initials.
- Hover stats bubble must look like the reference.
- Panel open/close must keep cat anchored.
- Motion must not feel random or jittery.
- Idle CPU must remain low.
- No network dependency for assets.
- No bundled fonts; use OS fonts.
- All strings must be i18n-ready.

## Required Contract Files

- `pet_motion_intent.md`
- `pet_state_machine.yaml`
- `motion_spec.yaml`
- `asset_manifest.json`
- `pet_tokens.json`
- `interaction_behavior.yaml`
- `schema_guide.md`
- JSON schemas in `schemas/`
- Reference image in `assets/reference/`
