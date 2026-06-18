# Repository map

File-by-file roles for ClipCat. This is **reference**, loaded on demand — it is
deliberately *not* part of the always-on `AGENTS.md` steering, so it never costs
context on every turn. Read it when you need to find where something lives; the
*why* lives in `AGENTS.md` (`## Architecture`) and the ADRs, this is the *where*.

```
src/
  main.rs              thin entry → clipcat::platform::run()
  lib.rs               module wiring
  pet.rs               Pet: platform-agnostic simulation, fish animation,
                       clip/panel orchestration + scene building
  clipboard.rs         ClipStore: clip history model + clips.json persistence
  panel.rs             clipboard panel UI state, dynamic Layout geometry
                       (user-movable/resizable card), hit testing, drag zones
  render.rs            all vector art (cat, fish, panel, accessories, bubble, icon)
  sysfont.rs           system-font text for everything drawn (ab_glyph,
                       ADR-0007/0011); hollow tofu box for uncovered glyphs
  hotkey.rs            panel-hotkey spec parsing ("win+shift+v") + display label
  i18n.rs              every user-visible string, English + Korean
  sound.rs             synthesized SFX; winmm on Windows, no-op elsewhere
  state.rs             Persist (JSON) + XP/level progression + accessory table
  update.rs            optional self-update (ADR-0009): daily GitHub release
                       check via system curl; exe swap + relaunch on Windows
  input.rs             shared atomic activity counters (KEYS/CLICKS/WHEEL)
  menu.rs              platform-agnostic context-menu model (entries, actions,
                       check state); Pet::build_menu / apply_menu_action drive
                       it. Native menus (macOS NSMenu) just render it — so the
                       whole menu is unit/e2e testable without a GUI.
  platform/
    mod.rs             selects exactly one backend by cfg
    windows.rs         native Win32 layered window + LL hooks + clipboard
                       listener + global panel hotkey (default Win+Shift+V,
                       fallback Ctrl+Shift+V) + Shell tray. The hotkey opens
                       the panel as a caret-anchored flyout in a *second*
                       focusable layered window (ADR-0013); middle-click keeps
                       the embedded panel.
    portable.rs        winit + softbuffer + rdev + arboard (macOS/Linux,
                       and Windows --feature portable). On macOS the hotkey
                       opens the panel as a caret-anchored flyout in a second
                       winit window (ADR-0013); Linux keeps the embedded panel.
    mac_input.rs       macOS-only global-input event tap (CoreGraphics).
                       Replaces rdev's keyboard listener, which crashes on
                       macOS 15 by calling Text Input Source APIs off the main
                       thread (LNR-0005); reads event kind + keycode only.
    mac_present.rs     macOS-only transparent presenter: pushes the pixmap to
                       the window's CALayer as a CGImage with alpha, so the pet
                       floats with a transparent background (ADR-0003 update)
                       instead of softbuffer's opaque card.
    mac_menu.rs        macOS-only native NSMenu shown on right-click; renders
                       the platform-agnostic menu::MenuEntry tree (submenus,
                       check marks, disabled items) and returns the chosen
                       menu::MenuAction. The portable tray-menu stand-in.
    mac_dialogs.rs     macOS-only NSAlert dialogs (About box, Reset-stats
                       confirmation) — the parity of the Windows MessageBoxW.
    mac_autostart.rs   macOS-only "run at login" via a ~/Library/LaunchAgents
                       plist (parity of the Windows HKCU\Run value).
    mac_caret.rs       macOS-only text-caret read for the flyout (ADR-0013):
                       Accessibility API (AXBoundsForRange), geometry only —
                       never element text; falls back to the mouse cursor.
  bin/gen_icon.rs      regenerates assets/clipcat.ico from render::draw_icon_scaled
examples/preview.rs    renders representative frames to PNGs (headless review)
tests/e2e.rs           end-to-end core flows through the public Pet API
tests/release_script.rs  e2e of scripts/release.sh in a scratch git repo (unix)
build.rs               embeds icon + version info on Windows hosts
scripts/release.sh     the release path: gates, bump, CHANGELOG rotation, tag,
                       push (scripts/release.cmd = Windows wrapper)
CHANGELOG.md           user-facing changes only — see policy in its header
assets/clipcat.ico     the embedded app icon — generated, not hand-edited
assets/screenshot.png  README screenshot (the only committed image asset)
docs/specs/            product & technical specs
.claude/skills/release/  project skill that drives scripts/release.sh
.context/kb/adr/       architecture decision records (why)
.context/kb/lnr/       lessons & near-misses (what bit us)
.github/workflows/     CI: builds windows + macos (no Linux build; changelog
                       lint runs on macos); v tags publish the GitHub release
                       + the binaries the in-app updater downloads
```
