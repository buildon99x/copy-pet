# LNR-0003: rdev global input — its own thread, plus macOS/Wayland caveats

- Date: 2026-06-12 · Area: portable input

## Symptom / risk

`rdev::listen` blocks the calling thread forever, and on macOS global event
taps want a CFRunLoop — a naive integration would either freeze the winit event
loop or fail to receive events.

## Cause

`rdev::listen` is a blocking call that installs an OS-level hook and runs a
loop. It must not share the thread that owns the winit event loop (the main
thread / UI loop).

## Fix

Spawn `rdev::listen` on a **dedicated background thread**
(`portable::spawn_global_input`); its callback only increments the shared atomic
counters in `input.rs` (so it needs no app state and is trivially `Send`). The
winit loop drains those counters each tick.

## Caveats (document for users, don't try to "fix" in code)

- **macOS:** global capture requires the user to grant Accessibility permission
  (System Settings → Privacy & Security → Accessibility).
- **Linux/Wayland:** Wayland deliberately blocks global input capture; rdev
  works under X11 only. The pet still runs; the activity loop just won't see
  input under Wayland.

## Takeaway

Treat global-input capture as a privileged, platform-gated capability. Keep the
hook callback dependency-free (atomics only) — it's the same discipline as the
Win32 `WH_*_LL` hooks, and it keeps the privacy guarantee easy to audit.
