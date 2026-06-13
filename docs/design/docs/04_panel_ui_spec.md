# 04. Clipboard Panel UI Spec

## Structure
Title row:
- Source filter button/funnel
- Capture pause/resume button
- Clear unpinned button with two-step confirm
- Language toggle EN/KO
- Close button

Search row:
- Search input with placeholder: EN `Search clips (text or source)` / KO `검색 (텍스트 또는 출처)`
- Active source filter chip inside search box.
- IME composition must render correctly; do not interpret composition keystrokes as shortcuts until committed.

List row anatomy:
- Pin star
- Quick-copy digit badge for top 10 visible rows
- Preview text, 1–2 lines
- Source badge/color dot + app name
- Relative time
- Size label for large clips
- Delete X with red hover halo

Footer:
- Count summary: `87 clips · 12 pinned`
- Shortcut hint: `Ctrl+0–9 Quick Copy · Ctrl+Z Undo`
- Actual registered hotkey: e.g. `Win+Shift+V` or fallback `Ctrl+Shift+V`

## Sorting/filtering
- Pinned clips first, then last copied timestamp descending.
- Re-copy existing clip bumps to top while preserving pin state.
- Search matches text + source app, case-insensitive, Unicode-aware.
- Korean search must work over Hangul strings.
- Source filter cycles: all -> distinct app list by most recent usage -> all.
- Unknown source clips visible only when filter = all.
- Reopening panel clears source filter but preserves query only if `persist_panel_query` option is later introduced; current default clears query.

## Keyboard behavior
- Type = search input.
- Up/Down select previous/next visible row.
- PageUp/PageDown jump one viewport.
- Home/End first/last visible row.
- Enter copy selected.
- Ctrl/Cmd+0..9 quick-copy visible badge row. 0 = topmost.
- Delete deletes selected and shows undo toast.
- Ctrl/Cmd+Z undo last delete/clear.
- Ctrl/Cmd+P pin/unpin selected.
- Tab cycle source filter.
- Esc order: disarm clear -> clear query -> clear source filter -> close panel.
- O toggles auto-close where portable shortcut is needed.

## Mouse behavior
- Row click copies clip and closes panel if auto-close on.
- Pin star toggles pin without copying.
- Delete X deletes without copying.
- Header drag moves panel offset relative to cat.
- Bottom-right grip resizes panel and persists geometry.
- Scroll wheel scrolls list, not page/window.

## Delete safety
- Single delete is undoable and toasts `CTRL+Z TO UNDO`.
- Undo stack stores last 20 delete/clear operations, session-only.
- Clear unpinned first press arms button, turns red, toasts confirm text.
- Any unrelated interaction disarms clear.
- Second press clears all unpinned clips and is undoable as one batch.

## Empty/error states
- No clips: show cat/fish empty illustration + `Copy text anywhere to feed ClipCat.`
- No search result: show `No matching clips` and clear query action.
- Capture paused: show amber paused chip in title row and muted fish icon.
- Hotkey clash: footer shows actual fallback hotkey; toast once.
- Permission missing macOS: show clear instruction bubble for Accessibility permission; keep local panel accessible by mouse if possible.
