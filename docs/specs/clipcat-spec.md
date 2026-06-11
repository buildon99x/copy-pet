# ClipCat — Product & Technical Spec

Status: implemented (v2.0) · Owner: ClipCat · Last updated: 2026-06-11

## 1. Summary

ClipCat (formerly DeskCat) is a **clipboard manager with a desktop pet as its
interface**. A small cat sits at the bottom of the screen and "types along"
with you (à la Bongo Cat). Every system-wide text copy is captured into a
local, searchable clipboard history; visually, a fish badged with the source
app flies into the cat's mouth. Keyboard/mouse activity and copies grant XP,
level the cat up and unlock cosmetic accessories. The whole UI is bilingual
(English / Korean). It is intentionally tiny, dependency-light and frameless.

References that shaped the design: **ClipClip** (clipboard history, pinned
clips, search, hotkey), **Bongo Cat** (reactive paw-tapping mascot) and
**Taskbar Hero** (passive productivity meter).

## 2. Goals / Non-goals

**Goals**
- Clipboard management as the core feature: capture, history, pin, search,
  copy-back, delete/clear, pause — end-to-end on all three OSes.
- Keep the v1 pet loop intact: input → reaction → XP → level → unlock.
- A delightful copy feedback: the fish + nom animation, badged per source app.
- English + Korean UI, switchable at runtime, Hangul rendered without font
  files.
- Single binary, persistence, tray/menu, autostart, graceful shutdown.
- Privacy-preserving: input contents never read; clips stored locally only;
  no network.

**Non-goals**
- Not a full ClipClip clone: text clips only (no images/files), no folders,
  no clip editing, no cloud sync, no paste-automation.
- Not a keylogger or analytics product; no telemetry.

## 3. Clipboard manager

### Capture
- **Windows (native):** `AddClipboardFormatListener` → `WM_CLIPBOARDUPDATE`;
  reads `CF_UNICODETEXT`. The source app is resolved via the clipboard owner
  (fallback: foreground window) → process image name; its real exe icon is
  extracted (`ExtractIconExW` → RGBA) for the fish badge.
- **macOS / Linux (portable):** an `arboard` watcher thread polls every
  ~400 ms (no change notification API). Source app is unknown there → the
  fish carries a colored initial badge instead.
- Both backends suppress exactly one event after writing the clipboard
  themselves (copy-back), via a last-set-text marker.
- Rules: empty/whitespace-only and > 256 KB texts are ignored; re-copying an
  existing clip bumps it to the top (keeping pin state). Capture can be
  paused/resumed (panel ⏸ button, tray menu); the setting persists.

### Store
- `clipboard::ClipStore`, persisted as `clips.json` in the config dir
  (saved on the existing 30 s dirty throttle + shutdown + panel mutations).
- Capacity: 100 unpinned (oldest evicted), 100 pinned (never evicted).
- A clip: id, full text, optional source-app name, pinned flag, unix
  timestamp of last copy.

### Panel (UI)
- Opens via: global **Ctrl+Shift+V** (Windows native, `RegisterHotKey`),
  middle-click on the cat (both backends), tray menu (Windows), `C` key
  (portable). The window grows upward (324×426 canvas at scale 1.0); the cat
  stays at the bottom.
- Contents: title row with capture-pause / clear-unpinned / language / close
  buttons; search box (live filter over text + source, Korean supported —
  IME input works on both backends); 6 visible rows (pin star · preview ·
  source + relative time · delete ✕) with hover + keyboard selection;
  scrollbar; footer with counts and the hotkey hint.
- Keyboard while open: type = search, ↑/↓/PgUp/PgDn = select, Enter = copy
  selected, Del = delete, Backspace = edit query, Esc = clear query / close.
- Clicking a row copies it back (toast "COPIED!", pop sound, happy cat).
  Pinned clips sort first.
- On the native backend the window is `WS_EX_NOACTIVATE`; opening the panel
  temporarily removes that style and focuses the window so search works,
  closing restores it.

## 4. Core loop (pet)

1. The user types/clicks anywhere → global hooks count events (never their
   content). The user copies → the clipboard watcher delivers the text.
2. Each ~33 ms tick the pet consumes counts + copy events: paws tap, an
   "excitement" value rises with input rate, and XP is granted:
   **2 XP/key, 1 XP/click, 1 XP/scroll, 5 XP/copy (+10 petting, +1 boop)**.
3. A copy also queues a **fish** (≤ 3 queued): it arcs from the top-right
   into the cat's opening mouth over ~0.9 s, badge showing the source app;
   on arrival: nom sound, sparkles + a heart, happy bump.
4. Crossing an XP threshold levels the pet up: star burst + chime, tray
   tooltip update, accessory unlock at set levels (see table).
5. Idle ≥ 75 s → sleep (Zzz); any input wakes it.

Progression math (`state::xp_to_next`): `200 + 80 · level²` XP per level,
clamped at level 99.

| Level | Unlock |
|------:|--------|
| 2 | Red scarf |
| 3 | Round glasses |
| 5 | Blue beanie |
| 7 | Headphones |
| 10 | Gold crown |
| 15 | Wizard hat |

## 5. Interactions

| Gesture | Effect |
|---------|--------|
| Copy anywhere | Fish + clip saved (+5 XP) |
| Ctrl+Shift+V / middle-click / tray / `C` | Toggle clipboard panel |
| Drag | Move the pet (unless position-locked) |
| Single click | Squash bounce + sparkle (+1 XP) |
| Double click | Pet it: heart burst (+10 XP) |
| Hover | Today's stats bubble (level/XP bar, keys, clicks, copies, active time) |
| Right click (native) | Context menu: clipboard, capture pause, stats, size, accessory, sound, lock, language, autostart, reset, about, quit |

## 6. Internationalization

- `i18n::Lang { En, Ko }`; every user-visible string lives in `i18n.rs`
  (tray menu, panel, toasts, bubble, about dialog). Accessory names carry
  both languages in `state::ACCESSORIES`.
- First run picks the language from the OS locale (`GetUserDefaultUILanguage`
  / `$LANG`); after that it's a persisted setting, switchable from the tray
  menu (Windows), the panel's EN/KO button, or `G` (portable).
- **Hangul rendering** (`hangul.rs`): syllables are decomposed
  (U+AC00 formula) and composed from ~40 vector jamo stroke shapes with
  standard vertical/horizontal/mixed vowel layouts, stroked by tiny-skia at
  any size — no font files (ADR-0006). ASCII stays the 5×7 bitmap font
  (now covering all printable ASCII incl. lowercase); unknown glyphs render
  a hollow box.

## 7. Platforms & backends

Same split as v1 (ADR-0001): a platform-agnostic core (`pet`, `clipboard`,
`panel`, `render`, `font`+`hangul`, `i18n`, `state`, `sound`) and exactly one
compiled backend — native Win32, or portable `winit`+`softbuffer`+`rdev`+
`arboard` (ADR-0005). Platform caveats: global input needs Accessibility on
macOS and X11 on Linux (Wayland blocks capture; clipboard then needs
XWayland); audio is Windows-only (ADR-0002).

## 8. Persistence

JSON at the per-OS config dir: Windows `%APPDATA%\ClipCat`, macOS
`~/Library/Application Support/ClipCat`, Linux `$XDG_CONFIG_HOME/ClipCat`.
`state.json` holds lifetime/daily counters (now incl. copies), window
position, language and all settings; `clips.json` holds the clip history.
Saved on a 30 s dirty throttle, on size change, drag-end and shutdown.
A v1 `DeskCat` dir (and the HKCU `DeskCat` autostart value on Windows) is
migrated automatically on first run.

## 9. Privacy

Input hooks only increment three atomic counters (`input::{KEYS,CLICKS,
WHEEL}`); no keycodes, characters, window titles or timings are stored.
Clipboard text is stored **locally only**, capture is pausable, oversized
clips are ignored, and there is no network code in the binary.

## 10. Quality bar

- Single binary, no installer, no bundled assets (icon, fonts and sounds
  generated from code).
- Idle/active CPU ≈ a few percent of one core (release); memory ~12–16 MB.
- `cargo clippy` clean (incl. `--features portable` and the
  `x86_64-pc-windows-msvc` target); `cargo test` green; CI builds on all
  three OSes; `cargo run --release --example preview` renders the review
  frames headlessly.
