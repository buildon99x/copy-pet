# 05. Exception Handling and UX Recovery

## Clipboard write-back suppression
When ClipCat copies a stored clip back to clipboard:
- Set `last_set_text_hash` before write.
- Suppress exactly one watcher event matching text/hash.
- If next event differs, capture normally.
- If OS delivers duplicate events, suppress only within 1500ms and only for exact match.

## Oversized text
- Ignore > 256 KB.
- Toast: `Clip too large — not saved` / `클립이 너무 커서 저장하지 않았어요`.
- Do not animate fish for ignored clip; optional tiny disappointed cat blink only.

## Empty/whitespace text
- Ignore silently by default.
- No fish, no XP.

## Storage failure
- If clips.json write fails, keep in memory and toast `Could not save clips`.
- Retry on next dirty throttle and shutdown.
- Atomic writes: temp file + fsync if feasible + rename.
- Corrupt clips.json: move to clips.corrupt.YYYYMMDDHHMMSS.json, start empty, toast once.

## Hotkey clash
Windows:
- Try configured hotkey.
- If clash, fallback Ctrl+Shift+V.
- UI must show actually registered shortcut.
macOS/portable:
- Chord tracker cannot reserve shortcut. If focused app consumes shortcut, middle click/tray/C fallback remains.

## Permission missing
macOS Accessibility missing:
- Pet still renders.
- Global input stats unavailable.
- Clipboard polling may continue if allowed.
- Show bubble: `Enable Accessibility for global shortcuts and typing reactions.`

## Multi-monitor and scale
- Anchor is stored in logical coordinates plus monitor identity when available.
- On startup clamp to active work area.
- If pet would be under taskbar/dock, raise above work area bottom.

## Auto-close off
Every copy path must preserve panel open:
- Row click
- Enter
- Ctrl/Cmd+digits
- Context action Copy
Selection remains on copied row; toast appears inside panel area.

## IME
- During Korean IME composition, do not trigger Enter copy unless composition is committed.
- Backspace edits composing text before query text.
- Search results update on committed text; optional composition preview may filter live only if platform API supports it cleanly.
