# LNR-0009: macOS flyout initialization is not focus loss

- Date: 2026-10-31 · Area: macOS portable clipboard flyout

## Symptom and evidence

Enter did not select/copy/paste from the hotkey flyout. With an isolated profile
and two synthetic clips, native event logging showed `Resized`,
`ScaleFactorChanged`, `Focused(false)`, then queued moves/redraw. The initial
focus callback closed the panel before any `KeyboardInput` reached it. Core
selection tests passed because they did not create Cocoa windows.

## Fix

Track whether the current flyout session has actually gained focus. Ignore the
initial loss callback and callbacks that disagree with current native focus.
Request keyboard focus on the next main-loop turn, after the hidden window is
shown. Re-enable IME on this window as well as the cat window.

Copy the selection before automatic paste, retain the external app PID before
showing the flyout, and wait for its activation without sleeping the UI thread.
Post V down/up with explicit Command flags: rdev's individually generated keys
do not reliably propagate Command to V. Missing Accessibility permission leaves
the selection copied and explains manual paste/the relevant setting. Embedded
panels must use their own current external app, never a previous flyout target.
Only app identity is observed, not app contents, window titles or user input.

## Regression checks

`flyout` unit tests cover initial/stale/real focus loss. Native CGEvent tests
cover Command flags without held hotkey modifiers. The actual Cocoa window flow
is exercised by `scripts/validate_macos_paste.swift`, in an isolated CI profile,
with an explicit skip report when Accessibility is unavailable. The native
probe must include a standard Edit/Paste menu: a bare custom NSTextView app
without this menu cannot receive ordinary Cmd+V, which would give a false
negative for correctly delivered paste events.
