# ADR-0021: Private local affection and bounded character motion

- Status: Accepted
- Date:2026-10-30
- Supersedes input/growth/body-click and cache-size portions of ADR-0017/0018.

## Decision

Remove all statistics, XP, levels and broad keyboard/mouse observation. Keep OS
registered hotkeys (ADR-0020), clipboard capture and optional output-only paste.
A shared core gesture accepts only local silhouette down/up, excluding panels,
transparent pixels, missing presses and movement beyond3 logical points. Native
Windows treats double-click notifications as another press; portable converts
physical cursor points once at its density boundary. Lock prevents movement,
never affection; cursor motion still cancels a locked drag. Capture loss, leave,
density changes and hidden state cancel pending gestures.

Affection uses4 independently authored eye-contact/gesture/contentment/settle
poses per breed. Durations vary0.78–1.16s; no sound, particles, count or log is
added. An active copy finishes at a safe cycle boundary, then pending affection
precedes decorative copy queues. At most one affection waits. Clips save before
animation arbitration. Pause clears existing effects and permits new local
responses; hide clears all effects. Selection also clears old character effects.

Append4 frames to the original16 without changing copy mouth tracks. One selected
20-frame cache is3,456,000 bytes (3.30MiB); the loader accepts20–24 complete cells
below4MiB. Thumbnails decode sequentially; reduced gallery previews keep their
compact16-frame contract. Replacement decode peaks are separate from retained
cache. Offline Rust packaging records source cuts, common breed scale and foot
baseline; generated source sheets stay outside the repository. No runtime network,
font bundle or heavy dependency pipeline is added. Stable frames do not repaint;
existing33ms platform/clipboard bookkeeping remains independent.

## Verification limits

Core gesture/state tests, art pixel preservation and budget tests, render previews,
release builds and cross-target checks cover reproducible behavior. Cross-checks
do not replace native Windows/macOS input, focus and mixed-display verification.
Actual Linux gestures/screenshots/performance are reviewed by the controller.
