# ADR-0010: Movable, resizable clipboard panel on a dynamic union canvas

- Status: Accepted
- Date: 2026-06-12
- Related: [ADR-0001](0001-cross-platform-architecture.md) (one window per
  backend, shared core), [ADR-0005](0005-clipboard-manager.md) (the panel)

## Context

The clipboard panel was a fixed 352×362 card hard-wired above the cat
(`panel.rs` exported the whole geometry as constants; the open-panel window
was always 360×542 at scale 1.0). Users asked to (a) resize the panel — more
history on screen — and (b) move the panel without moving the cat. Both
backends position a **single** OS window that contains cat + panel, so
"moving only the panel" needs either a second OS window (a large per-backend
surface area: layering, focus, click-through, two presenters) or a layout
model inside the existing window.

## Decision

- **Card geometry becomes user state**: `Panel { w, h, off }` — card size
  plus its offset *relative to the cat's top-left* — persisted in
  `state.json` (`panel_w/h`, `panel_off_x/y`, clamped on load by
  `panel::clamp_geometry`). `Panel::layout()` derives everything else
  (buttons, search, rows that fit, footer, canvas) on the fly; the old
  constants are gone.
- **The window canvas is the union** of the cat's 240×256 canvas and the
  margin-padded card. The cat's origin inside that canvas moves as the card
  moves; `Layout::cat` reports it.
- **The cat never moves on screen.** Every layout change (panel toggle,
  card move/resize, scale change) computes the shift of the cat's
  bottom-center anchor in physical pixels and accumulates it in the Pet;
  backends consume it via `Pet::take_window_shift()` together with
  `take_size_changed()` and offset the window by exactly that amount. This
  replaces the old bottom-center-anchored resize math in both backends (it
  reproduces it for scale changes and the default panel layout).
- **Drag zones**: the header strip (left of the buttons) moves the card,
  a bottom-right grip resizes it (`Panel::drag_hit` → `PanelDrag`).
  Backends feed `Pet::panel_drag_start/_update/_end`; deltas are tracked in
  **screen** pixels (divided by scale) because the window itself moves and
  resizes under the pointer mid-drag — window-local coordinates would feed
  back into themselves.

## Consequences

- ✅ Panel resize (280–600 × 220–700) and independent panel placement with
  zero new OS surface area; everything is core logic, unit/e2e-testable.
- ✅ The geometry contract between Pet and backends is two calls
  (`canvas_size` + `take_window_shift`) — both backends shrank their
  anchor math.
- ⚠️ Cat + panel still share one window: dragging the card grows the union
  canvas (clamped to ±480 canvas units offset), so an extreme placement
  means a large (mostly transparent on Windows) window. Acceptable; a
  second OS window remains the escalation path if ever needed.
- ⚠️ During a card drag the surface is recreated per tick (~30/s) like any
  OS window resize. Position writes go through the dirty flag + 30 s
  autosave, not per-tick disk writes.
