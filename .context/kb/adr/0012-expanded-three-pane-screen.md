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
- Because everything (window size, cat anchor, `take_window_shift`, drag
  bookkeeping) already flows through `Panel::layout()` (ADR-0010), the larger
  card reuses all of it unchanged: the cat stays anchored, centered below the
  card. The desktop pet is **still rendered** below the screen — this is an
  expansion of the pet app, not a separate productivity window.
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
  pet-first" product contract; the expanded screen is opt-in.
- The pet renders below the wide card, so the expanded window is tall
  (~768×824 at 1×). This is consistent with how the compact panel already
  places the cat, but differs from the standalone concept mock (which shows no
  desktop pet). Acceptable for an in-app expansion; revisit if a pet-less
  expanded window is wanted.
- First cut (this ADR) implements the Clipboard/Pinned views, the detail pane
  and Copy/Pin/Delete. Statistics / Customization / Settings nav destinations
  and the Edit-note / Open-source / Quick-copy detail actions are follow-ups.
