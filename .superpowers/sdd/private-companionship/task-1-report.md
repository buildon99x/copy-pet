# Task 1 implementation report

Date: 2026-10-30. Base: `84d98a6`. Branch: `feat/cat-breeds-motion`.

## Scope and result

Removed lifetime/daily key, click, copy and activity-minute statistics from `Persist`, plus XP, progression calculation, daily rollover, level reporting and level-up stars/chime/toasts. Serde continues accepting old fields as ignored unknown fields; the normal `Persist::save` serialization no longer writes any of them or the former bubble-pinned setting. Existing settings, stable cat selection and the separate clip/project storage paths remain intact. Normal saves do not reset user data.

Removed statistics bubble scene/rendering and previews, statistics/reset menu entries and platform handlers, and level/key totals from the English/Korean About text and tray tooltip. Removed obsolete progression/bubble/reset tests; retained copy reaction/history, pause, hide, gallery, image, paste and panel coverage and replaced affected counter assertions with functional history checks where appropriate. Changelog and immediately affected repository guidance now describe the new behavior.

`Pet::advance(k,c,wh)` remains temporarily source-compatible but ignores all three values. Broad input listeners/counters and hotkey registration are deliberately unchanged for Task 2. Ctrl+B workbench navigation remains in the portable key mapping, and existing local editing/paste paths remain unchanged.

The macOS smoke script now parses `clips.json` and checks exact persisted synthetic text entries, stable selected cat and omission of legacy XP/copy fields after normal autosave. It no longer uses statistical counters as evidence of clipboard capture.

## Test-first evidence

Added before implementation:

- `state::tests::legacy_statistics_are_discarded_on_normal_serialization`: reads an old state with every statistical field and checks those fields disappear while cat, lock, capture, custom hotkey and panel size survive serialization.
- `pet::tests::legacy_input_and_copy_preserve_settings_without_growth_effects`: reads old XP immediately below a level threshold, advances with activity, then copies synthetic text. It checks saved state remains unchanged, history contains the text, and no level-up toast/particles appear.

Ran `cargo test --locked --release legacy_ --lib` before production edits. Expected RED: 2 failed, 2 passed. The serialization check failed with `legacy field total_keys must not be saved`; activity check failed because one key changed keys/XP and daily state. Evidence: `/tmp/task1-red.log`. An initial compile attempt caught a private ClipStore field in the new test; this was corrected to its public `by_index` API before collecting RED evidence.

Ran the same focused command after implementation. GREEN: 4 passed, 0 failed. Evidence: `/tmp/task1-green.log`. Subsequent full verification covers the final implementation.

## Commands and outcomes

Each shell sourced `/workspace/.clipcat-env/env.sh`.

- `cargo test --locked --release`: PASS, 175 library + 31 e2e + 3 release-script tests = 209 tests; zero failures, plus empty binary/doc suites. No compiler warnings. This default Cargo test total excludes the five standalone `--example prepare_cat_atlas` tests; those were not run for this statistics/UI-removal task. Run once as the final full suite; `/tmp/task1-tests.log`.
- `cargo build --locked --release`: PASS; `/tmp/task1-build.log`.
- `cargo clippy --locked --release`: PASS with 8 existing warnings (6 library, 2 gen_icon). Locations are unchanged code in clipboard matching/sorting, portable presentation/event handling, badge RGBA premultiplication and icon generation. No new warnings introduced; `/tmp/task1-clippy.log`.
- `cargo check --locked --release --target x86_64-pc-windows-msvc`: PASS on the native Win32 backend; `/tmp/task1-windows.log`.
- `cargo check --locked --release`: passed during implementation; an intermediate unused stats-formatting helper warning was resolved by deleting that obsolete helper.
- `bash scripts/release.sh verify`: PASS, changelog/version checks only; no release action.
- Python compile of `scripts/validate_macos.py`: PASS. An initial `py_compile` cache was removed; no generated cache is committed.
- `git diff --check`: PASS.

## Self-review and limitations

Reviewed the full affected diff and searched for statistical/progression references. Remaining legacy names occur only in migration fixtures/macOS smoke seeds; Task 2 still owns input counters/hooks and their existing tests/comments. Copy history, tags/projects, pause, suppression, paste/focus ordering, cat atlases, fonts and Retina conversion code are unchanged. No dependency, lockfile or generated asset changes.

No Linux GUI smoke or screenshots were produced for this task: the controller explicitly requested consolidated actual-app smoke/screenshots after Task 3 to avoid duplicate work. No native macOS or Windows execution was available here; the Windows check is compilation only, and macOS smoke execution remains CI/manual coverage. No push, merge or release was performed.
