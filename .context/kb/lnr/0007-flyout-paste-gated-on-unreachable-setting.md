# LNR-0007: The Win+V flyout never pasted — gated on a setting unreachable on Windows

- Date: 2026-06-17 · Area: core pick contract + Windows backend (auto-paste / panel hotkey)

## Symptom

Opening the clipboard with the panel hotkey (Win+V parity) while typing in a
text field, then picking a clip — click or **Enter** — copied it but **never
pasted** it into the field. The clip landed on the clipboard; the user still had
to paste by hand. This is the *exact* headline parity gap with Win+V that the
flyout (ADR-0013) and auto-paste (ADR-0012) were built to close — so the feature
looked finished while its whole reason for existing was dead.

The plumbing was all present and correct: `WM_HOTKEY` captured the focused field
as `paste_target` (LNR-0006), the flyout opened at the caret, and `copy_back`
could `SetForegroundWindow` + `SendInput` Ctrl+V back into it. None of it ran.

## Cause

Two independent gaps lined up so the paste could never fire:

1. **The pick's `paste` bit was tied only to the opt-in setting.**
   `run_action(Copy)` set `ClipPick.paste = self.st.paste_on_select`, with no
   notion that the *flyout* is the Win+V path and should always paste. The
   middle-click embedded panel and the caret-anchored flyout — different
   gestures with different intents — shared one rule.
2. **That setting was unreachable on the release platform.**
   `paste_on_select` defaults **off** and lives in the shared `menu.rs` model,
   which only the macOS NSMenu renders. The Windows tray menu (`show_menu` in
   `platform/windows.rs`) is **hand-built** and silently omitted the toggle. So
   the one switch that could enable the paste was invisible on Windows — the
   "premium native" release target. Off by default, no way to turn it on → the
   flyout paste was unconditionally dead there.

Either gap alone would have been survivable (a default-on setting, or a
reachable toggle). Together they made a documented, shipped feature a no-op on
its primary platform, and neither gap was caught because **the flyout's intent
("a pick here pastes") was never asserted by a test** — only the embedded
`paste_on_select` path was.

## Fix

Decouple the flyout's intent from the opt-in setting in the core:

```rust
// run_action(Copy): the caret-anchored flyout is the Win+V path, so it
// always pastes; the embedded panel keeps the opt-in paste_on_select.
paste: self.st.paste_on_select || self.flyout,
```

Read `self.flyout` *before* `after_pick` (auto-close clears it). Locked with a
pick × source matrix test (`flyout_pick_always_pastes_for_win_v_parity` +
`paste_on_select_flag_flows_into_pick`). Linux/Windows-portable never open the
flyout for the hotkey (`#[cfg(target_os = "macos")]`), so `self.flyout` stays
false and their behavior is unchanged.

## Takeaway

- **A feature's headline intent must be a test, not just an ADR.** "Picking from
  the flyout pastes" was prose in ADR-0012/0013; nothing failed when it didn't.
  When a behavior is the *reason a feature exists*, assert it directly.
- **A setting that gates a behavior must be reachable on every platform that has
  the behavior.** `menu.rs` is the shared menu model, but the Windows tray menu
  is hand-rolled and can silently drop an entry — exactly the "one deliberate
  split" AGENTS.md warns about. When you add a `MenuAction`, surface it in the
  Windows tray menu too, or you ship a knob no Windows user can touch.
- **Don't let one rule serve two gestures with different intents.** Middle-click
  panel (opt-in copy/paste) and the caret-anchored flyout (Win+V, always paste)
  are different contracts; collapsing them into one bool hid the bug.
