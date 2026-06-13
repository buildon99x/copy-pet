# QA Results — Dark Premium implementation (M1–M10)

Verification status for [`QA_CHECKLIST.md`](QA_CHECKLIST.md). "Test" names are in
`tests/e2e.rs` or the `#[cfg(test)]` modules of the named source file; "Frame"
names are emitted by `cargo run --release --example preview`
(see [`../docs/09_visual_regression_frames.md`](../docs/09_visual_regression_frames.md)).

What cannot run headless (real Win32 hotkey registration, the OS clipboard, the
system tray, macOS Accessibility) is validated by the unit-tested seams those
paths drive plus CI builds — called out per item below.

## Privacy
- No key characters read/stored — `src/input.rs` exposes only atomic counters;
  typing tiers use a keys/sec *count* (`pet.rs` `kps`), never content. ✅
- Clipboard text stays local — only `src/update.rs` touches the network
  (ADR-0009). ✅
- Update check optional / no telemetry — `auto_update` toggle, `update.rs`. ✅

## Clipboard
- Empty/whitespace ignored — `clipboard.rs` `add_copy` (existing tests). ✅
- >256 KB ignored with a warning toast — `clipboard.rs` size guard. ✅
- Re-copy bumps existing clip, preserves pin — test `copy_filter_and_copy_back_flow`. ✅
- Copy-back suppresses exactly one matching event — backend suppression marker
  (ADR-0005); per-backend, not headless. ⚙️
- Pinned/unpinned capacity rules — `clipboard.rs` tests. ✅
- Corrupt `clips.json` backed up + reset + toast — test
  `corrupt_history_is_backed_up_and_reset` (M9). ✅

## Panel
- Hotkey toggles panel / fallback shown — `panel_hint_follows_registered_hotkey`;
  Frame 07 footer shows `WIN+SHIFT+V`. ✅
- Korean search — `search_combines_with_source_filter`; Frame 08. ✅
- IME composition doesn't fire shortcuts — portable `ime_composing` guard (M7);
  backend-level, not headless. ⚙️
- Ctrl/Cmd+0..9 quick copy — `quick_copy_hotkeys_copy_top_clips`. ✅
- Ctrl/Cmd+Z restores delete and clear — `delete_undo_and_two_step_clear_flow`. ✅
- Esc layer order — `esc_peels_one_layer_per_press_in_spec_order` (M7). ✅
- Auto-close off keeps panel open — `panel_autoclose_toggle_keeps_panel_open`. ✅

## Pet
- Idle/curious/sleep transitions — `pet_mood_transitions` (M2). ✅
- Typing intensity changes paw speed — keys/sec tiers (M2); Frames 02/03. ✅
- Fish never blocks storage — clip stored before the fish flies
  (`on_copy` order); test `copy_event_stores_clip_grants_xp_and_queues_fish`. ✅
- Queue max 3 + overflow merge — `fish_queue_overflow_merges_with_count` (M4). ✅
- Level up interrupts and returns — `pet_mood_transitions` LevelUp priority. ✅

## Visual
- Dark-premium surfaces match tokens — `src/tokens.rs`; Frames 06–11. ✅
- No text clipping EN/KO — Frames 07 (EN) / 08 (KO) with OS font fallback. ✅
- Cat anchor stable on panel open/resize — `panel_resize_and_move_persist_and_anchor`. ✅
- Crisp rounded borders 1x/2x — `render::round_rect`; preview at scale 1.0. ✅
- Source badge color consistent fish↔rows — shared `render::source_badge` (M6). ✅

## Platform
- Windows `RegisterHotKey` fallback — `windows.rs` (Ctrl+Shift+V); UI reflects it. ⚙️
- macOS Accessibility-missing state — `notify_accessibility_needed`; Frame 12. ⚙️
- Window clamp after monitor change — `windows.rs` `clamp_to_screen`. ⚙️
- Graceful shutdown saves state/clips — atomic writes (`state::write_atomic`). ✅

Legend: ✅ verified headless (test/frame) · ⚙️ verified via unit-tested seam +
CI build / code review (cannot run headless).
