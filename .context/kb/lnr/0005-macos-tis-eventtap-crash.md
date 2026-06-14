# LNR-0005: macOS 15 SIGTRAP — rdev translates keys (TIS) off the main thread

- Date: 2026-06-12 · Area: portable input (macOS)

## Symptom

On macOS 15 (Sequoia), pressing any key while ClipCat ran — famously Ctrl+C —
force-quit the app instantly with `EXC_BREAKPOINT (SIGTRAP)`. The crashing
thread was the global-input listener thread, not the UI thread:

```
Thread 2 Crashed:
0  libdispatch.dylib   _dispatch_assert_queue_fail
2  libdispatch.dylib   dispatch_assert_queue
3  HIToolbox           islGetInputSourceListWithAdditions
4  HIToolbox           isValidateInputSourceRef
5  HIToolbox           TSMGetInputSourceProperty
6  clipcat             (rdev convert → Keyboard::create_string_for_key)
7  SkyLight            processEventTapData
...
15 CoreFoundation      CFRunLoopRun      (rdev::listen's run loop)
```

## Cause

The portable backend ran `rdev::listen` on a dedicated thread (LNR-0003) for
the activity counters + panel-hotkey chord. rdev's macOS path
(`macos/common.rs::convert`) eagerly fills `Event.name` on **every key press**
by calling `Keyboard::create_string_for_key`, which calls the Text Input
Source Manager (`TISGetInputSourceProperty` / `TSMGetInputSourceProperty`) to
translate the keycode to a character.

Since macOS ~14.4 those TIS lookups go through `islGetInputSourceListWithAdditions`,
which now `dispatch_assert_queue(main)` — it **hard-aborts (SIGTRAP) whenever
called off the main dispatch queue.** rdev's tap callback runs on the listener
thread, so the first translated keystroke trips the assertion. The assertion
fires inside the list getter on every call, so it is not flaky and cannot be
"primed" away from the main thread.

Two traps for the next person:
- It is **not** an Accessibility-permission problem. The opposite: the
  permission must be *granted* for the tap to receive key events at all — and
  only then does the crash happen. Without it, no events, no crash, no feature.
- It is a hard `abort()`, not a Rust panic — `catch_unwind` cannot save you.
  The only fix is to never call TIS off the main thread.

## Fix

ClipCat never uses `Event.name`. So on macOS we stopped calling `rdev::listen`
and run our own minimal CoreGraphics event tap instead:
[`src/platform/mac_input.rs`](../../../src/platform/mac_input.rs). It reads
only the event *kind* and the raw keycode (mapped to `rdev::Key` with rdev's
own keycode table) and **never touches TIS** — so the assertion is never
reached. Linux keeps `rdev::listen` (X11 has no such API). If the tap can't be
created (permission missing), `listen` returns `Err` and the app surfaces a
one-time hint toast instead of dying.

## Takeaway

On macOS, **any HIToolbox/TIS/TSM call from a CGEventTap callback (or any
non-main thread) is a latent crash on modern macOS.** Keep tap callbacks to
raw CoreGraphics field reads only. When a dependency does more than you need on
a privileged hot path, owning a tiny slice of it can be safer than patching it
— and here it also tightened the privacy boundary (no key text is ever
produced; ADR-0008).

Note: this crash is specific to the *listen* path (rdev translating keys to
text). `rdev::simulate` — used for opt-in auto-paste ([ADR-0012](../adr/0012-auto-paste.md))
— is output-only (it posts a CGEvent, doesn't read or translate input), so it
does **not** go through the crashing TIS path and is safe on macOS.
