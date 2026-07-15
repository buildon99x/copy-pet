# LNR-0008: The Win+V flyout paste intermittently dropped — we hid the flyout before restoring focus

- Date: 2026-07-15 · Area: Windows backend (auto-paste / panel hotkey)

## Symptom

On Windows (release 2.2.0), typing in a text field, opening the clipboard with
the panel hotkey (the caret-anchored flyout, Win+V parity), then picking a clip
— **Enter or mouse click** — copied it to the clipboard but **sometimes did not
paste** it into the field. Intermittent: the same gesture worked most of the
time and silently failed other times. This is the third bug in this same
auto-paste/flyout feature after [LNR-0006](0006-auto-paste-foreground-capture-order.md)
(captured our own window as the target) and
[LNR-0007](0007-flyout-paste-gated-on-unreachable-setting.md) (paste gated on an
unreachable setting) — the plumbing from both was present and correct, yet the
paste still dropped.

Two configurations *never* failed, and that asymmetry was the tell:

- `panel_autoclose` **off** — the flyout stays up after a pick, so nothing hid.
- the **embedded** middle-click panel — it never hides its window before pasting.

## Cause

Two independent, compounding gaps.

1. **We `SW_HIDE`-d the flyout before restoring the target's foreground.**
   `after_flyout_action` (the default, autoclose-on path) did:

   ```rust
   if !self.pet.flyout_open() { ShowWindow(self.flyout_hwnd, SW_HIDE); } // hide FIRST
   if let Some(pick) = pick { self.copy_back(pick); }                    // then paste
   ```

   `SetForegroundWindow` is honored when the calling **process** owns the
   foreground — then it may pass the foreground to another app's window. But
   `ShowWindow(SW_HIDE)` on the *foreground* window makes Windows asynchronously
   hand the foreground to an arbitrary Z-order neighbour. By the time
   `copy_back` called `SetForegroundWindow(target)`, our process had often
   already lost the foreground, so the foreground lock silently refused the
   call and the synthesized Ctrl+V landed nowhere (or in the wrong window). The
   `AttachThreadInput` cure from LNR-0006 doesn't help once the foreground has
   left our process. The two never-failing configs are exactly the two that
   never hide before pasting — they call `copy_back` while still owning the
   foreground.

2. **A still-held Shift turned Ctrl+V into Ctrl+Shift+V.** The default hotkey is
   Win+Shift+V and the clash fallback is Ctrl+Shift+V, so Shift is down when the
   panel opens. `send_ctrl_v` injected a bare Ctrl+V with no modifier cleanup, so
   a fast pick (Enter / quick click) while Shift was still physically held was
   seen by the app as Ctrl+Shift+V — paste-special or an unbound no-op in many
   apps, i.e. another "copied but didn't paste."

Neither was caught because the backend ordering and the injected-keystroke shape
are Win32 side effects with no headless test seam — only the core `ClipPick.paste`
*bit* was asserted (the LNR-0007 guard), and that bit was already correct.

## Fix

- **Paste before hide.** `after_flyout_action` now runs `copy_back(pick)` first
  — restoring the target's foreground while the flyout still legitimately owns
  it — then hides the (now-background) flyout, which can't steal the foreground
  back. The `!flyout_open()` guard stays.
- **Neutralize stuck modifiers.** `send_ctrl_v` releases both sides of Shift and
  Alt (`VK_LSHIFT`/`VK_RSHIFT`/`VK_LMENU`/`VK_RMENU`, explicit L/R because
  `MOD_SHIFT`/`MOD_ALT` accept either side and a generic `VK_SHIFT`/`VK_MENU` up
  maps only to the left key) before the clean Ctrl+V. A key-up for a key that
  isn't down is a harmless no-op; it stays output-only (reads no key state). Win
  is left alone — a lone synthetic Win up can pop the Start menu.
- **Guard the capture against the flyout too.** `capture_paste_target` now
  excludes `self.flyout_hwnd` as well as `self.hwnd`, so a hotkey re-press while
  the flyout is dismissing can't stash the flyout itself as the target.
- **`IsWindow` hardening.** `copy_back` skips the paste when the captured target
  no longer exists.

Validated by `cargo check --target x86_64-pc-windows-msvc` + code review (no
Linux CI job; the Win32 ordering isn't headless-unit-testable) and the manual
both-triggers Windows matrix (AGENTS.md verify steps 4–5).

## Takeaway

- **Never `SW_HIDE` the window that owns the foreground before you've handed the
  foreground to your intended target.** Do the activation + paste while you still
  legitimately own the foreground; hide afterwards. Hiding first drops the
  foreground token to a random Z-order window and the foreground lock then
  refuses your `SetForegroundWindow`.
- **An intermittent bug with a config that never fails is pointing at the
  difference.** The autoclose-on/off (and embedded-vs-flyout) asymmetry *was* the
  root cause — enumerate what the working paths do that the failing one doesn't.
- **A synthesized shortcut inherits whatever modifiers the user is still
  holding.** If your feature is opened by a chord (Shift/Win/…), clear those
  modifiers before injecting the paste, or the keystroke you send isn't the one
  you meant. Residual: a higher-integrity (elevated) target still blocks
  `AttachThreadInput` + `SendInput` via UIPI — native Win+V has the same limit.
