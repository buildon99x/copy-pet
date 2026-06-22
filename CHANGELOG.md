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
- **13 new accessories to unlock** as your cat levels up: bunny ears, a sprout,
  a daisy crown, bear ears, cherries, a butterfly, heart sunglasses, a chick, a
  sleep mask, a nightcap, a fish hat, a fish-shaped pastry and — at the very top
  — a lucky four-leaf clover. Equip them from the tray/right-click menu like the
  others.
- **Pudding accessory** — a Japanese-style caramel pudding hat (custard body,
  caramel glaze with a drip, and a cherry on top) unlocks at level 57.

### Fixed
- **Copying a very large item no longer stalls the cat or spikes memory.** A
  huge text, HTML or RTF copy used to be read into memory in full before being
  discarded for exceeding the history's size limit; the clipboard read now
  stops at that limit, so an oversized copy is skipped cleanly (on Windows this
  also avoids a brief freeze while it was read). Images and files were already
  ignored and still are.
- **Picking a clip from the hotkey panel now pastes it** (Win+V parity, Windows):
  opening the clipboard with the panel hotkey while typing in a text field and
  then clicking a clip — or selecting it and pressing **Enter** — now pastes it
  straight into that field, instead of only copying it. (The separate
  "Paste on select" setting still controls the middle-click panel by the cat.)
- **The blue beanie's pom-pom no longer floats above the hat** — it now rests on
  the crown so it reads as a bobble hat instead of a detached dot over the cat.

### Changed
- **Korean stats bubble reads natively**: the cat's "active time" now shows as
  "1시간 35분" in Korean instead of the English "1H 35M".
- **macOS/Linux: the clipboard panel toggles on middle-button release**, matching
  the Windows behavior so the middle-click feels the same across platforms.

## [2.2.0] - 2026-06-17

### Added
- **Formatting is kept when you paste** (Win+V parity): copied rich text now
  keeps its original formatting (bold, colors, links) when you pick it from the
  panel, instead of always pasting as plain text. (Windows and macOS; on Linux
  clips stay plain text.)
- **"Paste as text" per clip**: click the new "..." on a clip row — or select
  the row and press the **→** arrow — to reveal **Paste as text** and **Delete**
  buttons. "Paste as text" strips the formatting and pastes the clean text;
  press **←** to hide the buttons again.
- **Tooltips on the panel's top icons**: hovering the view / filter / pause /
  clear / language / close buttons now shows a label so it's clear what each does.
- Hovering a clip's **★** now shows a tooltip with its keyboard shortcut
  (**Ctrl/Cmd+P** pins or unpins the selected clip), so the shortcut is
  discoverable without hunting through the footer.
- **Clipboard opens at your cursor** (Win+V parity): pressing the panel hotkey
  now pops the clip list up at the text caret in whatever app you're typing
  in, instead of over the cat — so it's right where you're working and the cat
  stays put. If the focused app doesn't expose a caret (some browsers/Electron
  apps), it opens at the mouse pointer. Middle-clicking the cat still shows the
  panel by the cat as before. (Windows and macOS.)
- **Auto-paste on select** (off by default): a new "Paste on select" setting
  (tray/right-click menu) makes picking a clip paste it straight into the app
  you were just in, instead of only copying it. On Windows it returns focus to
  that app and sends Ctrl+V; on macOS/Linux it's best-effort (pastes into
  whatever is frontmost after the panel closes). Leave it off for copy-only.
- **Change the panel hotkey from the menu**: the tray/right-click menu now has
  a "Panel hotkey" entry showing the current combo — click it to cycle through
  safe presets (Win/Cmd+Shift+V → Ctrl+Shift+V → Alt+Shift+V → Ctrl+Shift+C).
  It re-registers instantly and a toast confirms the new combo. (On Linux/
  portable, press **K** while the pet is focused.)
- **First-run hint**: until you open the clipboard panel for the first time,
  the cat shows a small banner with the exact hotkey (e.g. "Clipboard:
  WIN+SHIFT+V"), so the history is discoverable from the very first launch.
  It disappears for good once you've opened the panel.
- The **About box** now explains that clips paste back with their original
  formatting, and how to paste a clip as plain text.
- A **GitHub** entry in the context menu (just below "About ClipCat") opens
  the project page in your browser.
- The clipboard panel now opens in a roomier **card view** by default: clips
  become rounded cards that wrap onto two lines, so you see more of each one.
  Switch to the **compact list** with the list/card button in the panel's
  header — your choice is remembered.
- New **Window** setting (tray menu on Windows, right-click menu on macOS)
  with three modes: **Always on top** (the default), **Normal** — the pet can
  now sit behind other windows like an ordinary one — and **Hide**. A hidden
  pet comes back from its tray icon or by pressing the clipboard hotkey.
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
- The per-clip **pin (★) moved to the right** side of each row, and the row's
  delete moved into the new "..." menu (the always-visible delete "X" is gone) —
  a cleaner, less cluttered row.
- **Smarter panel search**: type several words and only clips matching
  *every* word are shown (the words can match the text or the source app),
  and results are ranked by relevance — matches at the start of a word, or
  where your whole phrase appears together, float to the top.
- The **clipboard hotkey (Win+Shift+V / ⌘+Shift+V) now always brings the panel
  to the front** instead of toggling it shut. If the pet is set to *Normal* and
  sitting behind other windows, or *Hidden*, pressing it reveals and focuses the
  panel every time. (Close the panel with Esc or its ✕ button.)
- **Each clip row shows more of its content.** Multi-line clips are flattened
  onto the row (so you see past the first line), the first few characters are
  bolded for quick scanning, and the text is a touch smaller to fit more in.
- The **clipboard panel now keeps one fixed (normal) size** no matter which
  pet size — small, normal or large — you choose. The cat still grows and
  shrinks; the panel stays comfortably readable.
- **The pet and the panel now move independently.** Dragging the cat slides
  only the cat — the open panel stays exactly where it is — just as dragging
  the panel's header already moved only the panel.
- The hover **stats bubble now uses your system font** instead of the
  built-in pixel font, matching the panel's typography.
- **Holding a key no longer farms stats**: key auto-repeat is ignored, so
  the key counter (and its XP) only advances once per actual press.

### Removed
- The built-in 5×7 pixel font and the hand-drawn vector Hangul are gone;
  every text surface renders with fonts read from your OS. (On a system
  with no usable font at all, text shows as placeholder boxes.)

### Fixed
- The clipboard panel **no longer opens off-screen**. When the cat sits near
  a screen edge — or its remembered panel position would reach past the
  monitor — the panel now slides itself fully into view as it opens, with the
  cat staying exactly where it is. You can still drag the panel partly off the
  edge yourself; it just won't get stuck there next time you open it.
- **The shown panel hotkey now matches what actually works.** When Windows
  reserves your configured combo (Win+Shift+V belongs to Windows' own
  clipboard history) ClipCat falls back to Ctrl+Shift+V — it used to do this
  silently, so the menu and hint showed one combo while a different one
  opened the panel. ClipCat now pops a short toast explaining the swap, and
  keeps your chosen combo saved so it works again once whatever holds it is
  freed (e.g. you turn off Windows clipboard history).

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
