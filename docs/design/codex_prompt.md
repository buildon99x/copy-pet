# Codex Prompt — Implement ClipCat Pet & Motion System

You are working in the ClipCat Rust repository.

Implement the pet appearance and motion system from the reference board using the contract files in `docs/design`.

Read first:

1. `docs/design/IMPLEMENTATION_BRIEF.md`
2. `docs/design/pet_motion_intent.md`
3. `docs/design/pet_state_machine.yaml`
4. `docs/design/motion_spec.yaml`
5. `docs/design/interaction_behavior.yaml`
6. `docs/design/pet_tokens.json`
7. `docs/design/asset_manifest.json`
8. `docs/design/schema_guide.md`

## Required Implementation

- Pet state machine
- Idle, typing slow, typing fast, typing extreme, sleep, yawn, look-around states
- Copy fish + badge + fly-to-mouth + nom + happy + XP popup sequence
- Hover stats bubble
- Petting reaction
- Boop reaction
- Level-up reaction
- New accessory reveal
- Panel open/close animation while cat remains anchored
- Accessory attachment and unlock rules
- Error handling for missing assets and animation drops

## Rust Guidance

Create or adapt these modules:

```txt
src/pet/
  mod.rs
  state_machine.rs
  motion.rs
  assets.rs
  renderer.rs
  particles.rs
  accessories.rs
  fish.rs
  hover_bubble.rs
```

## Rules

- Keep pet visuals deterministic for screenshot tests.
- Do not let the cat jump position when the panel opens.
- Do not queue more than 3 fish. Merge overflow into sparkle/XP feedback.
- Use delta-time based animation, not frame-count based animation.
- Support high-DPI scaling.
- Do not block UI thread on asset loading.
- Provide fallback vector cat if assets fail.
- Do not implement telemetry.

## Acceptance

- Reference states can be previewed from a development command.
- Copy event animation completes in the correct sequence.
- Hover bubble content matches runtime data.
- Petting gives +10 XP.
- Boop gives +1 XP.
- Copy gives +5 XP.
- Level-up interrupts lower-priority states.
- Visual QA frame export exists.
