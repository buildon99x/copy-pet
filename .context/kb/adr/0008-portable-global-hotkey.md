# ADR-0008: Global panel hotkey on the portable backend via rdev chord matching

- Status: Accepted
- Date: 2026-06-11
- Related: [ADR-0001](0001-cross-platform-architecture.md) (backend split),
  [ADR-0005](0005-clipboard-manager.md) (clipboard manager)

## Context

The clipboard panel's global hotkey (default Win+Shift+V) was Windows-only:
`RegisterHotKey` has no portable equivalent, and golden rule #1 said global
input hooks may *only* increment the activity counters — never inspect which
key was pressed. That left macOS/Linux users without the single most
important clipboard-manager gesture (the product owner explicitly asked for
Cmd+Shift+V on macOS); the window-local `C` key requires focusing the pet
first, which defeats the point of a clipboard popup.

The conflict: detecting a chord requires looking at key identity in the
global listener. On Windows the OS does this for us (`RegisterHotKey`
delivers only the registered chord); on the portable stack the only global
key source we have is the `rdev` listener we already run for counting.

## Decision

- The portable backend's rdev callback feeds every key event through a
  `ChordTracker` that compares it against **the one user-configured chord**
  (`Persist::hotkey`, the same spec the Windows backend registers) and
  raises an `AtomicBool` the main loop consumes into `Pet::toggle_panel`.
- The privacy boundary is narrowed, not removed, and is now written into
  golden rule #1: the tracker holds five booleans (four modifier states +
  main-key-down for auto-repeat suppression); key identities are compared
  and **immediately discarded — never stored, buffered, logged or
  transmitted**. No other key inspection is permitted in the listener.
- The `win` modifier in the hotkey spec means the OS super key: Windows
  key, ⌘ Command on macOS, Super on Linux. One default spec
  (`"win+shift+v"`) therefore reads Win+Shift+V / Cmd+Shift+V /
  Super+Shift+V; `hotkey::super_name()` localizes the label.
- The matcher requires the modifier set to match *exactly* (Ctrl+Shift+V
  does not trigger a Win+Shift+V chord, nor does Win+Ctrl+Shift+V) and
  fires once per physical press (auto-repeat suppressed), mirroring
  `MOD_NOREPEAT` on Windows.

## Consequences

- ✅ Interaction parity: all three OSes open the panel with the same
  configured chord; the panel footer shows the OS-correct label
  (`CMD+SHIFT+V / C` on macOS).
- ✅ Testable without a display: `ChordTracker` is pure state; unit tests
  cover exact-match, wrong/extra modifiers, auto-repeat and disarm.
- ⚠️ A deliberate, documented exception to "counters only". Any future
  change widening what the listener looks at must revisit this ADR — the
  rule remains: compare-and-discard against the configured chord, nothing
  else.
- ⚠️ Platform caveats are inherited from rdev: macOS needs the
  Accessibility permission (already required for counting); Wayland blocks
  global capture entirely, so the hotkey only works under X11/XWayland —
  middle-click and `C` remain the fallbacks.
- ⚠️ Unlike `RegisterHotKey`, the chord is not *reserved*: the focused app
  still receives the keystroke (e.g. Cmd+Shift+V pastes-without-style in
  some macOS editors while also toggling the panel). Acceptable; users can
  configure a different spec in `state.json`.
