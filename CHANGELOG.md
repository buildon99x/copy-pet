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
- macOS: right-click the cat to open a context menu (clipboard panel, size,
  accessory, sound, lock, language, quit) — the same actions as the keyboard
  shortcuts, now discoverable without the Windows-style tray icon.

### Changed
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
