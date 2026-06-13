# Changelog

All notable **user-facing** changes to ClipCat are documented in this file:
new features, behavior changes and bug fixes that users can see. Internal
work (refactors, CI, build tooling, docs, dev-environment changes) is
deliberately **not** listed here — that history lives in git.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versions follow [SemVer](https://semver.org/). Maintenance rules:

- Every PR with a user-visible change adds a bullet under **[Unreleased]**
  (`Added` / `Changed` / `Fixed` / `Removed`), written for users, not devs.
- `scripts/release.sh` turns **[Unreleased]** into the next version section
  (and refuses to release while it is empty); `scripts/release.sh verify`
  lints this file in CI.

## [Unreleased]

### Added
- A floating **"+5 XP / +10 XP" popup** now drifts up from the cat whenever a
  copied fish is eaten or you pet it, so the reward is visible.
- During copy bursts, extra copies now **merge into the latest fish with a
  "+N" count** instead of being dropped — every clip is still saved.
- The clipboard panel is now yours to arrange: drag its **header** to move
  the panel anywhere — the cat stays put — and drag the **grip in its
  bottom-right corner** to resize it (a taller panel shows more clips).
  Size and position are remembered across restarts.
- **Quick copy with Ctrl+0–9** (Cmd works on macOS): while the panel is
  open, the first ten rows wear small digit badges — pressing Ctrl plus
  that digit copies the clip instantly, following whatever search or
  app filter is active. The numpad digits work too.
- New setting **"Close panel after copy"** (tray/right-click menu; `O` key
  on Linux): switch it off and the panel stays open after copying, so you
  can grab several clips in a row. On by default.

### Changed
- **New dark-premium look**: the clipboard panel and stats bubble are now a
  dark glass card with gold highlights — gold selection borders, a gold XP
  bar, colored per-app source badges (matching the fish) and purple quick-copy
  digit badges. The cat keeps its warm look on a dark keyboard.
- The cat is **livelier**: it gets curious (a side glance and ear flicks) after
  a while idle, throws sparks while you type at full speed, and opens its mouth
  right as the fish arrives.
- A **double-click pet now grants exactly +10 XP** (the single-click bounce it
  starts with is no longer double-counted).
- The hover **stats bubble now uses your system font** instead of the
  built-in pixel font, matching the panel's typography.
- **Holding a key no longer farms stats**: key auto-repeat is ignored, so
  the key counter (and its XP) only advances once per actual press.

### Fixed
- **Korean (IME) input in search**: pressing Enter to confirm a composing
  Hangul syllable no longer also copies the selected clip and closes the panel.
- A **corrupt clipboard history file** is now backed up and the app starts
  fresh with a brief notice, instead of failing to restore your clips silently.

### Removed
- The built-in 5×7 pixel font and the hand-drawn vector Hangul are gone;
  every text surface renders with fonts read from your OS. (On a system
  with no usable font at all, text shows as placeholder boxes.)

## [2.1.0] - 2026-06-12

### Added
- Deleting a clip is now forgiving: every delete (and even "clear all") can
  be undone with **Ctrl+Z** while the panel is open, and the clear-all
  button asks for a confirming second press (it turns red and shows a toast
  first) instead of wiping the history on a single click.
- The clipboard panel is fully keyboard-driven: besides the existing
  arrows/PgUp/PgDn/Enter/Del/Tab/Esc, **Home/End** jump to the first/last
  clip, **Ctrl+P** pins/unpins the selected clip (the selection follows it
  as the list re-sorts) and **Ctrl+Z** restores deleted clips. A help line
  in the panel footer lists the shortcuts.
- Every panel row now shows its source app's color dot (the same color as
  that app's fish badge), so clips are recognizable by app at a glance;
  large clips also show their size next to the timestamp.
- ClipCat can now keep itself up to date: once a day it checks GitHub for a
  newer release and the cat shows a toast when one exists. On Windows, pick
  "Update to vX.Y.Z and restart" in the tray menu and ClipCat downloads the
  new version and restarts itself; on macOS/Linux press **U** to open the
  download page. The check is on by default, can be turned off via "Check
  for updates automatically" in the tray menu (or `auto_update` in
  `state.json`), and sends nothing — it only reads the latest version
  number from github.com.
- The clipboard-panel hotkey now also works globally on macOS and Linux:
  **Cmd+Shift+V** on macOS, Super+Shift+V on Linux (the `win` modifier in
  the configurable spec maps to the OS's super key). macOS needs the same
  Accessibility permission the cat already uses; on Linux it requires X11.
- Clipboard history can now be filtered by the app a clip was copied from:
  a funnel button in the panel header (or the Tab key) cycles through the
  source apps, the active filter shows as a chip in the search box, and Esc
  clears it. Search and the app filter combine.
- The panel hotkey is configurable via `hotkey` in `state.json`
  (e.g. `"ctrl+alt+c"`); invalid values fall back to the default.
- macOS: right-click the cat for a full context menu at parity with the
  Windows tray — clipboard panel, pause capture, always-show-stats, Size /
  Accessory / Sound / Language submenus (with check marks; locked accessories
  greyed until their level), lock position, run at login, automatic updates,
  reset stats (with a confirmation), About, an "update to vX.Y.Z" item when a
  release is found, and quit.

### Changed
- The clipboard panel is much roomier and easier to read: 8 visible rows
  (was 6), taller rows, larger text and bigger header buttons.
- Picking a clip (Enter or click) now closes the panel so you can paste
  right away — reopen it with the hotkey or middle-click.
- Hovering a row's delete ✕ highlights it in red, so destructive clicks
  are obvious before they happen.
- Settings and clipboard history are now written to disk atomically, so a
  crash or power loss mid-save can no longer corrupt them.
- The clipboard panel and toast messages now render in your system's UI font
  (Segoe UI / Malgun Gothic on Windows, with sensible equivalents on
  macOS/Linux) for much better readability. The cat's hover stats tooltip
  keeps the cozy pixel font. Nothing is bundled or downloaded — the font is
  read from the OS.
- The default clipboard-history hotkey is now **Win+Shift+V** (was
  Ctrl+Shift+V). If another app already owns the combination, ClipCat
  automatically falls back to Ctrl+Shift+V; the panel footer and tray menu
  always show the hotkey that is actually active.
- macOS: the cat now floats on your desktop with a transparent background,
  like the Windows build, instead of sitting on a solid rounded card.

### Fixed
- macOS: ClipCat no longer crashes the moment you press a key (e.g. Ctrl+C) on
  macOS 15. If the Accessibility permission needed for the global hotkey and
  the keyboard/mouse "tap along" hasn't been granted, the cat now shows a hint
  pointing you to System Settings and keeps running normally instead of failing
  silently.

## [2.0.0] - 2026-06-12

### Added
- ClipCat: every system-wide text copy is captured into a local, searchable
  clipboard history — the cat eats a fish badged with the source app.
- Clipboard panel (Ctrl+Shift+V or middle-click): search, pin, delete,
  clear, copy-back, keyboard navigation, capture pause.
- Source-app name and real app icon on the fish badge (Windows).
- Copies grant XP and count in the daily stats bubble.

### Changed
- Renamed from DeskCat to ClipCat; existing DeskCat settings and autostart
  registration migrate automatically on first launch.

## [1.0.0] - 2026-06-12

### Added
- DeskCat: a desktop pet cat that taps along with your typing and clicking,
  earns XP, levels up and unlocks accessories. Bilingual UI (English /
  Korean), tray menu, autostart, local-only persistence.
