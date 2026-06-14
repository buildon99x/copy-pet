# LNR-0006: Auto-paste pasted into nothing — we captured *ourselves* as the target

- Date: 2026-06-14 · Area: Windows backend (auto-paste / panel hotkey)

## Symptom

With "Paste on select" on, picking a clip from the panel **opened by the global
hotkey** (the Win+V-equivalent path) copied the clip but never pasted it into
the app the user came from. Opening the panel by **middle-clicking the cat** and
then picking pasted fine. The earlier fix ([commit `ac5b496`](../adr/0012-auto-paste.md))
that attached the input queues (`AttachThreadInput`) around the foreground
switch — the documented cure for the `SetForegroundWindow` + `SendInput` race —
did not help the hotkey path at all, which made it look like the race fix was
wrong.

## Cause

The race fix was fine. The bug was upstream of it: **`paste_target` held our own
window**, so `copy_back`'s guard `paste_target != self.hwnd` silently skipped the
paste entirely.

`paste_target` was captured in a single place — `App::apply_size`, at the
panel-open focus-steal:

```rust
if self.pet.panel_open() && no_activate {
    self.paste_target = GetForegroundWindow(); // who had focus before we steal it
    // ...remove WS_EX_NOACTIVATE, SetForegroundWindow(self), SetFocus(self)
}
```

`apply_size` runs a tick *later*, when `take_size_changed()` fires. That ordering
is fine for middle-click (our window is `WS_EX_NOACTIVATE`, so the click never
activated it — the real app is still foreground when `apply_size` reads it). But
the hotkey path calls `reveal()` **inside `WM_HOTKEY`, before that tick**, and
`reveal()` does `SetForegroundWindow(self.hwnd)`. So by the time `apply_size`
read `GetForegroundWindow()`, the answer was already **us**. `paste_target`
became our own hwnd and auto-paste was a no-op forever on the hotkey path.

The trap: the capture site looked correct in isolation ("grab the foreground
right before we steal it"), and the half that broke is the half hardest to test
on a headless box — so the `AttachThreadInput` fix shipped looking complete while
the headline (Win+V) path stayed dead.

## Fix

Capture the target **before** any code can foreground us, at the gesture itself,
and never let the capture store our own window:

- New `App::capture_paste_target()` reads `GetForegroundWindow()` and stores it
  only when it is non-null and `!= self.hwnd`.
- `WM_HOTKEY` calls it **before** `reveal()` (guarded by `!panel_open()`, so a
  re-press while the panel is up keeps the original target).
- `apply_size` calls the same guarded helper instead of a bare assignment, so on
  the hotkey path (where foreground is already us) it leaves the earlier capture
  intact, while the middle-click path still captures the live target there.
- `copy_back` additionally `SetFocus(target)` inside the attached block (belt &
  suspenders: nail keyboard focus onto the target within the shared input state
  before `SendInput`).

## Takeaway

When you stash "the window that had focus before us", capture it at the **user
gesture**, before *any* of your own `SetForegroundWindow`/`reveal` can run — not
at a later lifecycle step that a sibling code path may have already raced past.
A single capture site is not automatically correct for every way a feature is
triggered: enumerate the trigger paths (hotkey vs. click vs. menu) and check the
foreground state each one leaves behind. And always guard such a capture against
your own hwnd, so a re-entry can never poison it.
