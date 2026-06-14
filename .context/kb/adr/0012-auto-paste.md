# ADR-0012: Auto-paste on clip select (opt-in), via synthesized Ctrl/Cmd+V

- Status: Accepted
- Date: 2026-06-14
- Related: [ADR-0005](0005-clipboard-manager.md) (copy-back contract it extends),
  [ADR-0008](0008-portable-global-hotkey.md) (input-privacy posture),
  [LNR-0005](../lnr/0005-macos-tis-eventtap-crash.md) (macOS input crash path),
  [LNR-0006](../lnr/0006-auto-paste-foreground-capture-order.md) (the focus-capture-order bug)

## Context

The single most-expected behavior of a clipboard manager — and the one
parity gap against Windows' built-in Win+V — was that picking a clip only
*copied* it; the user still had to paste manually. ADR-0005's copy-back
contract handed the backend an `Option<String>` meaning "put this on the
clipboard"; it had no notion of pasting, and pasting needs OS input
synthesis, which the platform-agnostic core must not do.

We want optional auto-paste **without** a new dependency or weakening the
privacy golden rule (input hooks may only count activity / match the panel
chord; key contents are never read, stored or transmitted).

## Decision

1. **Extend the copy-back contract** (supersedes ADR-0005's `Option<String>`):
   panel picks return `Option<ClipPick { text, paste }>`. `paste` is set from
   the new persisted `paste_on_select` flag (default **off** — some users want
   copy-only, and some apps mispaste). The core still never touches the OS; it
   only carries one bit of intent.
2. **Output-only synthesis, no new dependency.** Windows native synthesizes
   Ctrl+V with `SendInput` (Win32, already linked); the portable backend uses
   `rdev::simulate` (already a dependency). We *inject* a keystroke, never read
   one — so golden rule 1 is intact, and because `simulate` is output-only it
   avoids the macOS TIS *listen* crash path (LNR-0005).
3. **Focus handling is per-backend.** Windows native captures
   `GetForegroundWindow()` at the panel-open *gesture* — before anything can
   foreground us — and never stores its own hwnd (the hotkey path reveals/
   foregrounds us a tick before the focus-steal runs, so it must grab the
   target up-front; see [LNR-0006](../lnr/0006-auto-paste-foreground-capture-order.md)).
   On paste it re-attaches the input queues, `SetForegroundWindow`s + `SetFocus`es
   back to that window, then synthesizes Ctrl+V — so the clip lands in the app
   the user came from. The portable backend cannot reliably re-focus the
   previous app, so paste there is **best-effort**: it lands in whatever is
   frontmost after the panel closes.

## Consequences

- ✅ The headline parity gap with Win+V is closed on the release target
  (Windows), opt-in and off by default.
- ✅ No new dependency; privacy posture unchanged (output-only, no key reads).
- ✅ If focus restoration fails the clip is still on the clipboard — graceful
  degrade to a manual paste.
- ⚠️ Portable focus restoration is unreliable (winit exposes no "focus the
  previous window"); macOS especially is best-effort. Documented as such.
- ⚠️ `SetForegroundWindow` is subject to Windows' foreground-lock rules; the
  target is captured *before* we steal focus and the panel auto-closes first
  to free the foreground, but a refusal degrades to manual paste.
