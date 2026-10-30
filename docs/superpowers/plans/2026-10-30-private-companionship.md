# Private companionship implementation plan

> **For agentic workers:** Use superpowers:subagent-driven-development, one implementer and task review at a time.

**Goal:** Remove usage statistics and broad input monitoring; add explicit breed-specific affection and use Control+Shift+V on macOS.

**Architecture:** Keep local window gestures in the backends and motion/state in Pet. Replace global input listeners with OS registered hotkeys. Decode additional affection art only for the selected cat.

**Tech Stack:** Rust, existing tiny-skia/winit/Win32 backends, native Carbon/Win32/X11 hotkey APIs.

**Spec:** [approved design](../specs/2026-10-30-private-companionship-design.md) (approved by user on 2026-10-30; levels/XP hidden confirmed).

## Global Constraints

- Remove all OS key/click/scroll/activity-minute counting, statistics UI, all XP/level growth and level-up effects. Do not count affection.
- Remove broad input listeners/hooks; no permission-sensitive fallback to a broad listener. Clipboard history and optional output-only paste remain.
- Preserve clips, tags, projects, cat selection, pause/resume, suppression, focus/paste ordering, Retina/font fixes, twelve free cats and the British cat's slim tail.
- macOS panel hotkey is Control+Shift+V; migrate the old default once, preserve other custom chords, explain registration failure. Keep standard in-app Command editing.
- Direct cat clicks only: silhouette down/up without drag; exclude panels and transparent regions; position lock does not disable affection. No reactions to typing or other apps' mouse activity.
- Breed-specific affectionate poses, quiet bounded 0.6–1.2s response, at most one queued reaction, safe interaction with copy/sleep/pause/hide. Selected detailed cache stays below 4MiB; no perpetual animation timer while stable.
- No new runtime network functionality, bundled fonts or heavy dependency pipeline. Use existing checkout/feature branch; do not merge or release. Commit and push when verified (prior user authorization).
- Actual screenshots and human-like use review are required; distinguish Linux actual use, render previews and macOS/Windows native CI/manual coverage.

## Review Focus

1. Legacy high-XP settings must not regain badges or lose clips/projects during migration (Task 1).
2. OS shortcut conflicts/denied permissions must leave menu/middle-click access and not revive input listeners (Task 2).
3. Different Mac custom hotkeys and Korean input must survive migration/rebinding (Task 2).
4. Press-drag-release, locked position, transparent area, panel actions and high DPI must not create spurious affection (Task 3).
5. Affection during copy/pause/sleep/hide must neither lose captured clips nor leave endless animation or retained frame caches (Task 3).

### Task 1: Remove statistics and growth

**Files:** `src/state.rs`, `src/pet.rs`, tests, `src/render.rs`, `src/i18n.rs`, `src/menu.rs`, both backends' statistics UI, examples, `scripts/validate_macos.py`.
**Consumes:** Existing Persist/Pet/public event API.
**Produces:** Persist accepts legacy statistical fields but stops writing them; no user-visible statistics, XP, level, or growth effects. Keep existing `Pet::advance(k,c,wh)` signature temporarily if that avoids hotkey-task overlap; values ignored and no input statistics are stored. Task 2 removes arguments/counters and global listeners.

- [ ] Write/run meaningful failing legacy-state serialization and no-growth/copy-history tests.
- [ ] Remove all statistics/growth processing and UI; preserve functional data/settings and non-statistics shortcuts such as Ctrl+B workbench. A normal successful save omits legacy stats/XP. Do not reset user data.
- [ ] Update Mac smoke checks to real clip contents/persistence instead of XP/copy counters; update directly affected examples/tests and visible English/Korean copy.
- [ ] Focused tests then full `cargo test --locked --release`, release build, cross-target Windows check where affected. Record evidence and commit scoped changes; do not push yet.

### Task 2: OS registered hotkeys without input monitoring

**Files:** `src/hotkey.rs`, `src/state.rs`, `src/input.rs`, `src/platform/{portable,windows,mac_input,mod}.rs`, new focused OS hotkey module(s), `Cargo.toml` comments only unless needed, tests.
**Consumes:** Task 1 no-statistics Pet/state; preserved paste simulation and local key handlers.
**Produces:** Native hotkey lifetime with registration/rebind/error handling; no all-input listeners; Pet ticks no longer consume input counters; Mac default and one-time migration to Control+Shift+V.

- [ ] Write/run failing migration, custom chord preservation and rebinding/state tests that exercise real pure policy code.
- [ ] Windows native keeps RegisterHotKey and deletes WH_KEYBOARD_LL/WH_MOUSE_LL. Portable Windows uses a registered hotkey on a message loop; Linux X11 uses XGrabKey (account for lock modifiers, errors, shutdown). Mac uses RegisterEventHotKey with main-loop-safe callback and teardown; never CGEventTap/rdev::listen. Wayland must fail explicitly with alternate panel access, no new unsupported claim.
- [ ] Use a small backend-owned hotkey bridge and existing event-proxy mechanism for UI requests. Keep optional paste rdev::simulate separate. No broad key state tracking for hotkey detection. Surface registration failure with existing localized UI, preserve menu/middle click access.
- [ ] Migrate only Mac old-default chord once via a persisted version/provenance marker; preserve other custom chords and later intentional changes. Labels, presets, help and docs agree with actual registration. Input monitoring permission is not required for opening panel; accessibility for optional paste/caret remains separate.
- [ ] Remove now-unused counters/listener modules and tests rather than retaining disabled security-sensitive code. Run focused/full tests, release build, Clippy and Windows native+portable cross checks; commit and report. Actual Mac compile/GUI is verified in CI after final push.

### Task 3: Local affection gestures, breed art and bounded motion

**Files:** `src/{pet,cat,hunter_art,render,cat_picker}.rs`, new focused gesture/motion helper if useful, `src/platform/{windows,portable}.rs`, `assets/cats`, atlas preparation tool, motion preview examples/tests; user-facing documentation/ADR/changelog/AGENTS.
**Consumes:** No-statistics Pet and OS-only hotkeys; existing 16 copy/idle frames, selected-art ownership and logical density boundary.
**Produces:** Intentional local click reaction and independently authored affection poses for all twelve cats; frame roles/durations/anchors and up to 24 detailed frames. Existing copy art and character proportions preserved.

- [ ] Write/run failing gesture/motion tests: silhouette and release, drag threshold in logical points, locked movement still permits affection, panel/background excluded; repeated clicks bounded; concurrent copy persistence, waking, pause/hide cleanup, settled repaint false.
- [ ] Implement backend local gesture capture; `Pet::pet`/`click_bounce` become explicit local events only. Preserve click/drag/focus behavior in native Windows and portable Mac/Linux. Avoid new global mouse observation for transparency.
- [ ] Add dedicated eye-contact/affection/settle frames (4–8 per cat), not copy-fish poses. Root may supply generated source art artifacts outside repository. Use image_gen for creative art/editing and existing offline extraction/normalization for packaging. First visually check Ragdoll/Siamese/Bombay/British then remaining eight. Keep author timing differences and recognizable anatomy/coat/curled ears/slim tail.
- [ ] Respond in about 0.6–1.2s, at most one pending affection, no per-click log/count, no sound/particle spam. Explicit affection precedes decorative copy at safe frame boundary; actual clips save immediately. Sleep wakes on affection/copy only, pause permits local affection, hide clears queued effects. Stable states stop repainting; enforce selected cache under 4MiB, sequential thumbnail decoding, variable frame count support.
- [ ] Update project instructions/specs/privacy and controls docs, new ADR/index, Unreleased changelog and actual UI examples. Remove stale active documentation describing statistics/input permissions; historical archives/ADRs may remain with supersession notes.
- [ ] Run focused/full tests, release build, Clippy, previews (view PNGs), Windows native+portable checks. Self-review, commit, report. Controller then performs actual Linux use/screen capture/performance tests, final review and Mac/Windows CI.

## Final verification

- [ ] Actual app: all twelve affection responses, single click vs drag/panel/background, copy during affection, pause/resume/hide/sleep, hotkey rebind/conflict, legacy state and no global input counters/listeners, 1x/2x previews and three sizes.
- [ ] Release CPU/RSS comparison and selected-cache accounting; save screenshots/GIF and report under `/workspace/artifacts/clipcat-private-companionship`.
- [ ] Whole-change review, fix verified issues, push existing feature branch, wait for Mac/Windows CI and universal Mac test build. No production release requested.
