# LNR-0008: Win32 WH_*_LL hooks must live on their own thread, not the UI thread

- Date: 2026-06-22 · Area: Windows backend (input)

## Symptom / risk

The native backend installed `WH_KEYBOARD_LL` / `WH_MOUSE_LL` on the same
thread that pumps the window message loop. That thread also services
`WM_CLIPBOARDUPDATE` (a copy read with up to 5×`Sleep(15)` retries plus
`OpenProcess` / `QueryFullProcessImageNameW` / `ExtractIconExW` icon
extraction), the ~33 ms `WM_TIMER` render tick, and the paste path. While the
thread sits in any of those handlers it cannot service the hooks — so a copy
burst, another app holding the clipboard, or a heavy repaint can briefly
**stutter system-wide keyboard and mouse input**, undercount activity, and on
older Windows get the hook torn down so the pet stops reacting.

## Cause

A low-level hook procedure is called **on the thread that installed it**, and
that thread must keep pumping messages. Windows waits up to
`LowLevelHooksTimeout` (HKCU\Control Panel\Desktop, default ~300 ms) for the
hook to be serviced, then skips that event (older builds silently unhook). The
hook *callback* being trivial (atomics only) is not enough — what matters is
that the hosting **thread stays responsive**, and the UI thread is not.

## Fix

Host the two hooks on a **dedicated thread** (`windows::InputHooks`) whose only
job is to install them, pump a `GetMessage` loop, and let the callbacks bump
the lock-free counters in `input.rs`. The UI thread drains those counters on
its tick. Teardown posts `WM_QUIT` to that thread (id captured at startup,
after a `PeekMessage` forces the queue to exist so the quit can't be dropped)
and joins it; the thread unhooks on itself. This is the same isolation the
portable backend already gets from its dedicated `rdev::listen` thread
([LNR-0003](0003-rdev-winit-threading.md)) — the native path just hadn't
applied it. The panel hotkey is unaffected: it uses `RegisterHotKey` →
`WM_HOTKEY` on the UI thread, so the LL hooks carry no chord state and the move
needs no new cross-thread communication.

## Takeaway

Global input hooks belong on a thread that does nothing else. "The callback is
cheap" is a trap — a low-level hook is only as responsive as the thread hosting
it, so never share that thread with clipboard I/O, icon extraction or
rendering. The atomic counters in `input.rs` exist precisely so the hook can
run anywhere and the UI thread reads the result later.
