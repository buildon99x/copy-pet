# Clipboard Workflow Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement task-by-task. Steps use checkbox syntax.

**Goal:** Ship the accepted capture privacy, ordered paste and image-text search improvements in that priority order.
**Architecture:** Extend shared clipboard, Pet, panel and menu contracts; place OS capture, hotkey and recognition integration under platform/. Keep async results bounded and acknowledged.
**Tech Stack:** Existing Rust/serde/tiny-skia and native OS APIs. Avoid new runtime dependencies.
**Spec:** docs/specs/clipboard-workflows.md

## Global Constraints

- Existing checkout; preserve the user's untracked .DS_Store. Use an isolated branch, no worktree.
- No input hooks, activity statistics, network services, language-model downloads or bundled fonts.
- English/Korean strings belong in i18n.rs; preserve legacy JSON and copy suppression.
- Windows native is primary; don't claim actual OS verification from cargo check.
- Commit/push each unit over SSH. Do not release or promote production.

## Review Focus

- Excluded copies must not leak via feedback, pending captures, undo, queue or OCR.
- Expiry and source changes must invalidate visible panel row caches correctly.
- Queue must not advance on failed native output or paste into ClipCat itself.
- Hotkey registration collision/teardown must leave the original panel hotkey usable.
- OCR must bound resources and discard stale results without blocking UI or fetching URLs.

### Task 1: Capture privacy and retention

**Files:** Modify src/state.rs, src/clipboard.rs, src/pet.rs, src/pet_tests.rs, src/menu.rs, src/i18n.rs, src/platform/{windows,portable,mac_clipboard}.rs; create focused privacy/platform helper modules as needed. Update README, CHANGELOG and product spec.
**Interfaces:** Persist adds excluded_sources: Vec<String> and retention_days: u16 (0/1/7/30). ClipStore exposes expire_before(cutoff: u64) -> bool; Pet applies privacy before add_copy_rich/add_image. Capture includes the source app name gathered at event time; backend marker checks precede payload reads. Shared menu provides toggles for observed/excluded sources and retention; Windows tray must expose them too.
- [ ] Write failing behavioral tests: excluded text/image produces no stored clip or feedback; case-insensitive source match; legacy settings remain valid; nonpinned expired entries and undo snapshots removed while pins survive; pause/resume stays silent. Run focused tests and record expected failure.
- [ ] Implement the shared policy, bounded source settings and periodic expiration, localized accessible menus and source identity/OS marker capture. Do not read user window titles or values.
- [ ] Run focused tests then cargo build --locked --release, cargo clippy --locked --release, cargo test --locked --release. Generate/inspect changed previews and record real versus unavailable OS verification.
- [ ] Self-review, update documentation with limits, commit feature. Controller reviews and pushes over SSH before Task 2.

### Task 2: Ordered paste queue

**Files:** Create src/paste_queue.rs; modify src/lib.rs, src/{pet,pet_tests,panel,render,i18n,menu}.rs and platform hotkey/output modules; update preview example if needed, README/CHANGELOG/spec.
**Interfaces:** Session-only bounded queue of Clip snapshots keyed by stable id; APIs for toggle membership, move, reset, next/previous and acknowledged output. Pet supplies a pending queue pick with ClipPick, and backend reports write/injection success before advancing. A dedicated configurable/preset registered shortcut is scoped to active queue; no Ctrl/Cmd+V interception. Reuse Task 1 source policy and invalidation. Extend the existing portable hotkey manager with distinct panel/queue slots in ONE native connection/handler: Carbon must dispatch by registered HotkeyId; X11 must retain singular ownership of its process-wide error-handler bridge. Do not instantiate two independent managers that overwrite the existing static X11 handler state.
- [ ] Write failing tests for selection order, reorder/remove/reset/replay, failed/successful acknowledgment, rich/image fidelity, combine order and image rejection, delete/expiry/exclusion invalidation, and legacy hotkey behavior. Run focused tests before implementation.
- [ ] Implement queue and localized discoverable panel/menu controls including count/order and preview. Keep all panel controls usable by keyboard; register dedicated next-paste notifications on Windows, macOS and X11 (explicit Wayland failure). Preserve focus and suppression.
- [ ] Run focused tests then complete release build/Clippy/tests; inspect rendered EN/KO previews, exercise possible real app paths and report OS limits.
- [ ] Self-review, update docs, commit feature. Controller reviews and pushes over SSH before Task 3.

### Task 3: Windows local OCR and image text search

**Files:** Create bounded recognition worker and Windows recognition adapter under src/platform/ (optional small script embedded as include_str!); modify src/clipboard.rs, src/{pet,pet_tests,panel,render,i18n,state,menu}.rs and src/platform/windows.rs; add focused tests, README/CHANGELOG/spec, ADR and index for native recognition boundary.
**Interfaces:** Add optional OCR text metadata with serde defaults, separate from image identity; ClipStore applies results by stable id plus image identity only if still retained; existing filtered_indices searches it. Explicit recognize/copy-text actions. Windows worker returns bounded success/empty/failure messages to UI; use installed OS OCR, no network or large dependency. Optional auto-index if robust; unavailable platforms expose no OCR action.
- [ ] Write failing tests for image text query/persistence, old clips, unchanged identity/dedup, extracted text copy, stale results after removal/expiry/exclusion, empty/unavailable/failure/retry, bounded worker requests/output. Run and record RED.
- [ ] Implement local Windows recognition, async bounded lifecycle and localized UI. Provide explicit result preview and error handling. If a subprocess is used it must have bounded stdin/stdout, no shell interpolation of clip contents, timeout and cleanup, and no temp image outside the user's private app directory.
- [ ] Run focused and full release build/Clippy/tests, previews, Windows native/portable checks and available real app flow. Add actual Windows OCR synthetic-image test/CI coverage where possible.
- [ ] Self-review, update docs/ADR and verification evidence, commit feature. Controller reviews, pushes SSH, observes CI, runs final review and reports limits.
