# ADR-0013: Caret-anchored clipboard flyout in a separate OS window

- Status: Accepted
- Date: 2026-06-14
- Related: [ADR-0010](0010-movable-resizable-panel.md) (this realizes its named
  "second OS window for the panel" escalation path),
  [ADR-0001](0001-cross-platform-architecture.md) (one core, per-backend
  windows), [ADR-0005](0005-clipboard-manager.md) (the panel + copy-back),
  [ADR-0003](0003-portable-rendering-card.md) (the macOS transparent CALayer
  present the flyout reuses), [ADR-0012](0012-auto-paste.md) (the paste-on-pick
  contract the flyout dismiss-then-paste builds on)

## Context

The clipboard panel opened **attached to the cat**: cat + panel share one OS
window whose canvas is the union of both (ADR-0010). The most-expected Win+V
behavior is for the hotkey to pop the clip list up **at the text caret** of
the app you're typing in — and for the cat **not to move** while it does. With
one shared window, "panel far from the cat, cat unmoved" is exactly the
"second OS window for the panel" that ADR-0010 deferred as its escalation path.

We want this on Windows (release target) and macOS, without a new dependency
and without weakening the privacy golden rule (input is counted, never read).

## Decision

1. **A separate, caret-anchored flyout window — dual model.** The hotkey opens
   the panel in its own window at the caret; **middle-clicking the cat keeps
   the embedded (cat-window) panel** unchanged (ADR-0010 union canvas, all its
   tests intact). This is additive: the flyout reuses the pure `Panel`
   hit-test / click / nav / wheel / search functions verbatim.
2. **Standalone layout in the core.** `Panel::layout_standalone()` lays the
   card at a small margin origin in its own canvas (card + margins, covering
   the resize grip); `Panel::active_layout()` (and a `standalone` flag) routes
   all geometry/hit-testing through it when the panel owns a window. New `Pet`
   seams: `open_flyout` / `close_flyout` / `flyout_open` / `flyout_size` /
   `render_flyout` (+ `take_flyout_resized`). `canvas_size`/`origin`/`draw` are
   guarded on `!flyout`, so the **cat window keeps its plain cat-only size and
   never relayouts** for the flyout — the cat does not move.
3. **Caret detection, geometry only, with a mouse fallback.** Windows reads the
   focused thread's caret via `GetGUIThreadInfo` → `rcCaret` → `ClientToScreen`;
   macOS reads it via the Accessibility API (`AXFocusedUIElement` →
   `AXSelectedTextRange` → `AXBoundsForRange`). Both read **only the caret
   rectangle** — never element text, value, window title or any content — and
   fall back to the mouse cursor when no caret is exposed (Chromium/Electron/
   UWP/Java). This is consistent with golden rule 1 and the input event tap's
   posture (LNR-0005). macOS already requires Accessibility permission for the
   event tap, so no new prompt.
4. **Per-backend second window, no new dependency.** Windows native creates a
   second **focusable** layered popup window (shares the class + wndproc,
   routed by hwnd; `GetGUIThreadInfo`/`ClientToScreen` are in already-enabled
   windows-sys features). macOS creates a second winit window with its own
   transparent `mac_present` CALayer presenter (ADR-0003), and reads the caret
   via pure ApplicationServices/CoreFoundation FFI + the existing `objc`. The
   flyout takes keyboard focus for search; **Esc, focus-loss and a clip pick
   dismiss it**, hiding the window before the paste so focus returns to the
   source app (ADR-0012). Full interaction parity: click, keyboard nav incl.
   Ctrl/Cmd+0-9, search, header-drag move (moves the window) and grip-drag
   resize (size is shared/persisted; the flyout position is ephemeral, derived
   from the caret each open).
5. **Linux is unchanged.** All flyout code is `#[cfg(target_os = "macos")]` in
   the portable backend; Linux/Windows-portable keep the embedded panel for the
   hotkey (no reliable cross-app caret there).

## Consequences

- ✅ Win+V parity: the hotkey opens the clip list at the caret with the cat
  pinned in place, on Windows and macOS — the headline placement gap closed.
- ✅ No new crate (windows-sys symbols already enabled; macOS AX/AppKit via FFI
  + existing `objc`/`core-graphics`); privacy posture unchanged (caret reads
  are geometry only).
- ✅ The embedded union-canvas panel (middle-click) and its ~15 tests are
  untouched; the flyout is core-tested headless (standalone layout + the cat
  window staying untouched while a flyout is up).
- ⚠️ Caret detection is best-effort: apps without a Win32 caret / AX caret fall
  back to the mouse cursor (expected, documented).
- ⚠️ macOS positions the flyout in physical pixels scaled by the **main**
  screen's backing factor; on a mixed-DPI secondary monitor the anchor is
  approximate (the monitor-fit still keeps the card on screen). The macOS path
  is validated by CI build + review, not local runtime (no Mac in the dev/CI
  loop here) — see the verification note in the PR.
- ⚠️ Two top-level windows per backend (cat + flyout). The flyout is created
  once (Windows) / lazily (macOS) and hidden when idle, so the steady-state
  footprint is unchanged.
