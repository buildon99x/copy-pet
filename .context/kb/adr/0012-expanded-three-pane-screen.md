# ADR-0012: Expanded three-pane screen as a panel mode

Date: 2026-06-13
Status: Accepted

## Context

The dark-premium design package ships a "full app concept" screen
(`docs/design/assets/png_preview/a_clean_dark_themed_desktop_application_ui_screens.png`):
a 760×560 three-pane productivity window — a left **sidebar** (logo, level/XP,
today's activity, nav: Clipboard / Pinned / Statistics / Customization /
Settings, capture status), a center **clip list**, and a right **clip detail**
pane (preview + Copy/Pin/Delete actions + Created/Source/Size/Type metadata).

The package itself scopes this as a *concept*, distinct from the runtime: doc
02 reads "Panel default canvas: **760×560 logical px in full app concept**,
compact runtime panel **360×542**", and doc 01's core screens are the pet, the
compact panel, the hover bubble and the tray menu. So the compact panel
(ADR-0010) is the contract; the big screen was not part of milestones M1–M10.

A maintainer asked for the full screen anyway, **as an addition** that keeps the
compact panel ("확장 모드로 추가").

## Decision

Add the expanded screen as a second **mode of the existing panel**, not a new
OS window or a new architecture:

- `Panel` gains `expanded: bool` and `nav: NavView`. In expanded mode
  `Panel::layout()` returns a fixed 760×560 card at a fixed offset
  (`EXP_W/EXP_H/EXP_OFF`) instead of the user's resizable compact geometry,
  which is kept separately and restored on collapse.
- The window still flows through `Panel::layout()` (ADR-0010), but in expanded
  mode `layout()` short-circuits to a **pet-less landscape window**
  (`EXP_W`x`EXP_H`, the card inset by `EXP_PAD`, `cat = (0,0)`): the desktop pet
  is **not drawn** and the three-pane card fills the whole window, matching the
  reference's standalone app screen. `Pet::draw` skips the cat scene entirely
  when expanded. (The first cut rendered the cat anchored below the card, which
  read as a different window from the reference — corrected here.)
- `Panel::expanded_layout()` derives the sidebar/list/detail sub-rects;
  `render::draw_expanded_panel` draws them; `Panel::expanded_hit` +
  `Pet::expanded_click` route interactions (collapse, nav switch, row select,
  detail Copy/Pin/Delete). The clip store, search, selection and undo are the
  **same state** the compact panel uses — only the chrome differs.
- Entry/exit is `Pet::toggle_expanded`, surfaced as `MenuAction::ToggleExpanded`
  in the shared menu model and the Windows tray (parity), plus the in-screen
  collapse button and `Esc`. Copying never auto-closes the expanded screen (its
  detail pane is the point).

## Consequences

- No new dependency, window, or data format; the compact panel and its tests
  are untouched. The mode is one bool plus a parallel render/hit path.
- The compact panel remains the default, preserving the "small, quiet,
  pet-first" product contract; the expanded screen is opt-in and, while shown,
  the pet steps aside so the window reads like the reference app.
- Remaining differences from the high-DPI concept mock are inherent, not
  layout: the single-weight system font (no true bold), colored-initial source
  badges instead of bundled app-logo icons (real icons are extracted only on
  the Windows native backend), and no OS window chrome / global top search bar
  (ClipCat is a frameless floating window). These are font/asset/policy limits,
  not the three-pane structure.
- First cut (this ADR) implements the Clipboard/Pinned views, the detail pane
  and Copy/Pin/Delete. Statistics / Customization / Settings nav destinations
  and the Edit-note / Open-source / Quick-copy detail actions are follow-ups.
