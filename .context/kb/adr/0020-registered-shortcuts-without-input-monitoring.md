# ADR-0020: OS registered shortcuts without input monitoring

- Status: Accepted
- Date: 2026-10-30
- Supersedes: ADR-0008; global input portions of ADR-0001

## Decision

Remove global keyboard/mouse listeners, held-key maps and activity counters.
Windows uses RegisterHotKey (native window or portable worker message queue).
macOS uses Carbon RegisterEventHotKey with a main-thread handler and owned
teardown. X11 uses XGrabKey on a private worker connection, including Caps/Num
lock masks, synchronous conflict detection and partial-grab cleanup. Xlib's
process-wide error handler catches only the private connection and delegates
other errors. Portable notifications wake winit through its event proxy.
Rebinding unregisters the previous shortcut; the displayed live label reflects
only a successful registration. A rejected chord may use the existing
Control+Shift+V fallback, with an explanation. Complete failure explains local
panel access. Clipboard capture and optional output-only paste remain separate.

macOS defaults to Control+Shift+V. A persisted hotkey_version marker migrates
only the legacy Command+Shift+V chord once; other custom shortcuts and later
intentional changes survive. Opening the panel needs no Input Monitoring or
Accessibility permission. Optional paste/caret accessibility stays independent.
Standard in-app Command editing is unchanged.

## Limits

Wayland sessions explicitly reject global registration, including XWayland;
middle-click, local C and the macOS/Windows menus remain available as applicable.
macOS uses physical ANSI keycodes without text-source APIs. macOS and Windows
runtime behavior needs actual OS/CI validation; cross checks are compile evidence.
Historical event-tap advice in LNR-0003/0005 describes removed code and must not
be reinstated. rdev is retained exclusively for output-only paste simulation.
