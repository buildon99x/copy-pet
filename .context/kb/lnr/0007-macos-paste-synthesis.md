# LNR-0007: macOS auto-paste did nothing — wrong modifier flag and a main-thread sleep

- Date: 2026-06-17 · Area: portable backend (auto-paste / panel hotkey, macOS)

## Symptom

On macOS, with "Paste on select" on, the clip flyout opened at the caret (the
[LNR-0006]/ADR-0013 fix worked) and picking a clip copied it — but it never
pasted into the focused field. The Windows path with the same setting pasted
fine, so the capture/restore *logic* looked right; only the macOS keystroke was
dead.

## Cause

Two independent macOS-only bugs, both in the paste-back keystroke path
(`src/platform/portable.rs`):

1. **The Command flag never rode on the `V` event.** The synthesis used
   `rdev::simulate` (`paste_synthesize`). rdev 0.5.3's macOS backend builds the
   `V` CGEvent with `CGEvent::new_keyboard_event(src, code, true)` and an empty
   flags field, relying on a *separately posted* `MetaLeft` keydown to supply the
   modifier. macOS only reliably recognizes Cmd+V when the Command flag is set on
   the **`V` CGEvent itself** (`set_flags(CGEventFlagCommand)`), so the shortcut
   was flaky / ignored.

2. **The target app was never frontmost when the key was posted.**
   `set_clipboard` called `mac_focus::activate_app(pid)` and then
   `std::thread::sleep(60ms)` on the winit / NSApplication **main thread**.
   `activateWithOptions:` is asynchronous — it needs a run-loop turn to take
   effect — and sleeping the main thread blocks that exact run loop. So the
   activation hadn't happened yet; the synthesized Cmd+V went to our own app (or
   nowhere) instead of the original caret. The 60 ms "give it a moment" was
   actively counter-productive: it was 60 ms of *not letting the activation run*.

The trap with #2: a `sleep` reads like "wait for the async thing to finish," but
on the thread that *drives* the async thing it does the opposite — it starves it.

## Fix

- **Set the flag, post natively.** Replace the macOS branch with a `core_graphics`
  keystroke (`paste_cmd_v`): `CGEventSource::new(HIDSystemState)`, then a
  key-down and key-up `CGEvent::new_keyboard_event(src, kVK_ANSI_V, _)`, each with
  `set_flags(CGEventFlagCommand)` and `post(CGEventTapLocation::HID)`. rdev stays
  for the Linux/Windows-portable path.
- **Defer instead of sleep.** `set_clipboard` now calls `activate_app(pid)` and
  schedules `pending_paste = Some(Instant::now() + 80ms)` — no sleep. The winit
  loop (`about_to_wait`) fires `paste_cmd_v()` once that instant passes, after the
  run loop has had its turns to actually bring the target app forward. Control
  flow wakes early for the scheduled paste so the delay holds even when idle.

This is the macOS analogue of the Windows `SetForegroundWindow` + `SendInput`
sequence, minus the blocking.

## Takeaway

- On macOS, put the modifier flag on the keystroke's own CGEvent — don't trust a
  separately-posted modifier keydown (rdev does the latter and it's unreliable for
  shortcuts).
- Never `sleep` on the main run-loop thread to "wait for" an async AppKit call
  (`activateWithOptions:`, and friends): the sleep blocks the very loop that would
  complete it. Yield to the run loop and act on the next turn instead — schedule a
  deferred action, don't block.
