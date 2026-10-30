# Cottonball Hunter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Implement the selected illustrated ragdoll and a natural copy reaction with bounded rendering cost.

**Architecture:** A cached transparent atlas supplies normalized illustrations. A deterministic motion timeline selects frames; Pet routes existing events and supplies redraw hints. Both native and portable renderers share the same core.

**Tech Stack:** Rust, tiny-skia, existing Win32 and winit backends.

**Spec:** `docs/superpowers/specs/2026-10-30-cottonball-hunter-design.md`

## Global Constraints

- No new runtime dependency or network feature.
- Bound cached frames below 4 MiB.
- Preserve collection, counters, XP, pause/resume synchronization, source fish, accessories and panel contracts.
- Keep existing checkout; use feature branch `feat/cottonball-hunter-animation`.
- User authorization already covers implementation and earlier commit/push and screenshot reporting preferences persist.

## Review Focus

- Bursts must store every accepted clip while limiting pending animation.
- Pause and hidden windows must not replay old copy reactions when restored.
- Static sleeping/idle art must stop repainting without starving hotkeys and panels.
- Frame extraction must not clip ears, tails or paws, or wobble the baseline.
- Accessories, notification text and fish must fit the illustrated face at all three scales.

### Task 1: Cached illustrated frames

**Files:** `assets/cottonball-hunter.png`, `src/hunter_art.rs`, `src/render.rs`, `src/lib.rs`, ADR-0017, preview example.

**Interfaces:** `hunter_art::draw(&mut Pixmap, usize, Transform)` draws a cached 240×180 frame. `Scene.hunter_frame: usize` and `Scene.capture_paused: bool` select art and persistent pause indicator.

- [x] Write/run failing cache transparency, frame bounds and memory tests.
- [x] Implement atlas loading, stable placement and illustrated rendering; adapt accessories/fish/toast placement.
- [x] Generate and inspect frames, scales and panel previews; tests pass.

### Task 2: Event timeline and redraw policy

**Files:** `src/hunter_motion.rs`, `src/pet.rs`, `src/pet_tests.rs`, `src/platform/portable.rs`, `tests/e2e.rs`.

**Interfaces:** `hunter_motion::frame(copy_progress: Option<f32>, happy: f32, sleep: f32, blink: f32) -> usize` chooses frame 0–15. Pet draws that frame and returns redraw only when visible animation/UI changes.

- [x] Write/run failing timing, burst, pause and stable redraw tests against the real Pet paths.
- [x] Implement twelve-stage copy cycle, bounded coalescing, stop sleep particles, retain required UI invalidations and hidden-window behavior.
- [x] Run targeted and complete release tests, build, Clippy, Windows checks and release verification.

### Task 3: Real use, performance and delivery

**Files:** usage documentation, CHANGELOG, repository map, artifact report outside repository.

- [x] Run actual app with isolated synthetic profile, exercise capture/burst/pause/panels/petting and fix material findings.
- [x] Measure release idle/event CPU and memory against previous binary; capture screenshots and an animation artifact.
- [x] Obtain fresh code review, resolve material findings with regression coverage, commit and push the feature branch.
