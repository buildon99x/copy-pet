# ClipCat — Product & Technical Spec

Status: implemented (v2.0 + unreleased 2.1 features) · Owner: ClipCat ·
Last updated: 2026-06-12

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
- English + Korean UI, switchable at runtime, Hangul rendered through the
  OS's own fonts (nothing bundled).
- Single binary, persistence, tray/menu, autostart, graceful shutdown.
- Privacy-preserving: input contents never read; clips stored locally only;
  no network beyond the optional update check (§10).

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
  Writes are atomic (temp file + rename), like `state.json`, so a crash
  mid-write never corrupts the history.
- Capacity: 100 unpinned (oldest evicted), 100 pinned (never evicted).
- A clip: id, full text, optional source-app name, pinned flag, unix
  timestamp of last copy.

### Panel (UI)
- Opens via a global hotkey on all OSes — **Win+Shift+V** by default, where
  the `win` modifier is the OS super key, so it reads **Cmd+Shift+V on
  macOS** and Super+Shift+V on Linux. Configurable as the `hotkey` spec
  string in `state.json` (`hotkey.rs` parses `"win+shift+v"`-style values;
  invalid values reset to the default on load).
  - Windows (native): `RegisterHotKey`; on a clash with another app it
    falls back to **Ctrl+Shift+V**. The panel footer, tray menu and About
    dialog always show the combination that actually registered.
  - macOS/Linux (portable): a `ChordTracker` on the existing rdev listener
    matches the configured chord and toggles the panel (ADR-0008 — exact
    modifier match, auto-repeat fires once, key identities compared and
    immediately discarded). Needs macOS Accessibility / X11 like the
    counters; unlike `RegisterHotKey` the chord is not reserved from the
    focused app.
  - Also: middle-click on the cat (both backends), tray menu (Windows),
    `C` key with the window focused (portable). The window grows around
    the cat (360×542 canvas at the default card size, scale 1.0); the cat
    itself never moves on screen.
- **Movable + resizable card** (`panel::Layout`): dragging the header strip
  moves the card independently of the cat; dragging the bottom-right grip
  resizes it (280–600 × 220–700 canvas units; more rows fit a taller card).
  The card's size and its offset relative to the cat persist in
  `state.json` (`panel_w/h`, `panel_off_x/y`; clamped on load). The window
  canvas is the union of the cat canvas and the card, and every layout
  change ships a window-position shift (`Pet::take_window_shift`) so the
  cat stays anchored while the window resizes around it.
- Contents: title row with source-filter / capture-pause / clear-unpinned /
  language / close buttons; search box (live filter over text + source,
  Korean supported — IME input works on both backends); as many rows as
  fit the card height (8 at the default size: pin star · preview · source
  color dot + app + relative time + size for large clips · quick-copy
  digit badge on the first ten rows · delete ✕ with a red hover halo)
  with hover + keyboard selection; scrollbar; footer with counts, a
  keyboard-shortcut help line and the hotkey hint; resize grip.
- **Source-app filter**: the funnel button (or Tab) cycles all → app 1 →
  app 2 → … → all over the distinct source apps in the history (most
  recently used first). The active filter renders as a chip (with the
  app's badge color dot) inside the search box and combines with the text
  query; reopening the panel clears it. Clips without a known source only
  show when no filter is active. Each row carries the same per-app color
  dot the fish badge uses, so apps are recognizable at a glance.
- Keyboard while open: type = search, ↑/↓/PgUp/PgDn/Home/End = select,
  Enter = copy selected, **Ctrl+0..9 = quick-copy the badged top rows
  (0 = topmost; follows the active search/filter; numpad works too)**,
  Del = delete (undoable), Ctrl+Z = undo delete/clear, Ctrl+P (Cmd also
  works on macOS, including for the digits) = pin, Backspace = edit query,
  Tab = cycle source filter, Esc = disarm clear → clear query → clear
  filter → close (one layer per press).
- **Delete safety**: every delete shows a "CTRL+Z TO UNDO" toast and is
  restorable (up to the last 20 delete/clear operations, session-only);
  the header clear-all button needs a confirming second press (it arms,
  turns red and toasts first — any other interaction disarms it).
- Clicking a row copies it back (toast "COPIED!", pop sound, happy cat)
  and closes the panel so the user can paste. **Auto-close is a setting**
  ("Close panel after copy", `panel_autoclose` in `state.json`, default
  on; tray/context menu on Windows+macOS, `O` key on portable): switched
  off, every copy path (click, Enter, Ctrl+digits) leaves the panel open
  for grabbing several clips in a row. Pinned clips sort first; pinning
  via Ctrl+P keeps the selection on the clip as it re-sorts.
- On the native backend the window is `WS_EX_NOACTIVATE`; opening the panel
  temporarily removes that style and focuses the window so search works,
  closing restores it.

## 4. Core loop (pet)

1. The user types/clicks anywhere → global hooks count events (never their
   content; a held key counts once — OS auto-repeat is filtered out, so
   holding a key does not farm stats or XP). The user copies → the
   clipboard watcher delivers the text.
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
| Win+Shift+V (default) / middle-click / tray / `C` | Toggle clipboard panel |
| Funnel button / Tab (panel open) | Cycle the source-app filter |
| Enter / row click (panel open) | Copy the clip back (+ close the panel, unless auto-close is off) |
| Ctrl+0..9 (panel open) | Quick-copy the row wearing that digit badge |
| Del / ✕ (panel open) | Delete the clip (Ctrl+Z undoes) |
| Ctrl+P (panel open) | Pin/unpin the selected clip |
| Trash button ×2 (panel open) | Clear unpinned clips (second press confirms) |
| Drag panel header (panel open) | Move the panel card — the cat stays put |
| Drag panel grip (panel open) | Resize the panel card (size persists) |
| Drag | Move the pet (unless position-locked) |
| Single click | Squash bounce + sparkle (+1 XP) |
| Double click | Pet it: heart burst (+10 XP) |
| Hover | Today's stats bubble (level/XP bar, keys, clicks, copies, active time) |
| `U` (portable, after the update toast) | Open the new release's download page |
| Right click (native) | Context menu: update (when found), clipboard, capture pause, panel auto-close, stats, size, accessory, sound, lock, language, autostart, auto-update toggle, reset, about, quit |

## 6. Internationalization

- `i18n::Lang { En, Ko }`; every user-visible string lives in `i18n.rs`
  (tray menu, panel, toasts, bubble, about dialog). Accessory names carry
  both languages in `state::ACCESSORIES`.
- First run picks the language from the OS locale (`GetUserDefaultUILanguage`
  / `$LANG`); after that it's a persisted setting, switchable from the tray
  menu (Windows), the panel's EN/KO button, or `G` (portable).
- **Text rendering**: every drawn surface — panel, toast, stats bubble,
  fish badge letters, Zzz — uses **the system font** (`sysfont.rs`,
  ADR-0007/0011): the OS UI font plus a Hangul-capable fallback are loaded
  at startup from well-known font files (Windows: Segoe UI + Malgun
  Gothic; macOS: SF/Helvetica + Apple SD Gothic Neo; Linux: Noto/DejaVu +
  Noto CJK/Nanum) and rasterized with `ab_glyph`. Nothing is bundled or
  downloaded. Characters no loaded font covers — and systems with no
  usable font at all — draw as a hollow "tofu" box (the former built-in
  5×7 pixel font and vector Hangul were removed; ADR-0011 supersedes
  ADR-0006).

## 7. Platforms & backends

Same split as v1 (ADR-0001): a platform-agnostic core (`pet`, `clipboard`,
`panel`, `render`, `sysfont`, `i18n`, `state`, `sound`) and exactly one
compiled backend — native Win32, or portable `winit`+`softbuffer`+`rdev`+
`arboard` (ADR-0005). Platform caveats: global input needs Accessibility on
macOS and X11 on Linux (Wayland blocks capture; clipboard then needs
XWayland); audio is Windows-only (ADR-0002).

## 8. Persistence

JSON at the per-OS config dir: Windows `%APPDATA%\ClipCat`, macOS
`~/Library/Application Support/ClipCat`, Linux `$XDG_CONFIG_HOME/ClipCat`.
`state.json` holds lifetime/daily counters (now incl. copies), window
position, language, the panel hotkey spec, the panel card geometry
(`panel_w/h`, `panel_off_x/y`), the auto-close flag and all other
settings; `clips.json` holds the clip history.
Saved on a 30 s dirty throttle, on size change, drag-end and shutdown.
A v1 `DeskCat` dir (and the HKCU `DeskCat` autostart value on Windows) is
migrated automatically on first run.

## 9. Privacy

Input hooks increment three atomic counters (`input::{KEYS,CLICKS,WHEEL}`)
and — on the portable backend — additionally compare each key event against
the one configured panel-hotkey chord, discarding it immediately (ADR-0008;
Windows uses the OS's own `RegisterHotKey` instead). For auto-repeat
suppression each keycode is additionally reduced to a held/released bit
(Windows: a 256-bit bitmap in `input.rs`; portable: the `KeyGate` set of
currently held keys) and discarded — nothing persists past the key
release. Beyond that, no keycodes, characters, window titles or timings
are read or stored.
Clipboard text is stored **locally only**, capture is pausable, oversized
clips are ignored, and the only network code is the update check below —
which transmits nothing beyond the request itself.

## 10. Auto-update

Premise: releases are git tags (`scripts/release.sh`) and CI publishes each
`vX.Y.Z` tag as a GitHub Release with stable asset names (ADR-0009).

- A background thread (`update.rs`) checks once a day (first check ~10 s
  after launch): `GET <repo>/releases/latest` via the **system `curl`**;
  the newest tag is read from the redirect URL — no API, no JSON, no new
  dependencies. A strictly newer semver toasts "NEW VERSION vX.Y.Z!" once
  per version.
- Gated by `auto_update` in `state.json` (default on); Windows tray menu:
  "Check for updates automatically".
- Applying is always user-initiated:
  - **Windows (native)**: tray "Update to vX.Y.Z and restart" downloads
    `clipcat-windows-x86_64.exe` on a worker thread (toast "DOWNLOADING
    UPDATE..."), verifies the PE magic, renames the running exe to
    `<exe>.old` (cleaned next start), copies the new one into place and
    relaunches via a detached helper.
  - **Portable (macOS/Linux)**: `U` with the window focused opens the
    releases page in the browser.
- Any failure toasts "UPDATE FAILED", rolls the exe back and keeps the
  menu entry for a retry.

## 11. Quality bar

- Single binary, no installer, no bundled assets (icon and sounds generated
  from code; all text uses fonts read from the OS at runtime, never
  shipped).
- Releases are cut by `scripts/release.sh` (CHANGELOG-gated; see
  `CHANGELOG.md` for the user-facing-only policy).
- Idle/active CPU ≈ a few percent of one core (release); memory ~12–16 MB.
- `cargo clippy` clean (incl. `--features portable` and the
  `x86_64-pc-windows-msvc` target); `cargo test` green; CI builds Windows
  and macOS (Linux is deliberately not built in CI); `cargo run --release
  --example preview` renders the review frames headlessly.
