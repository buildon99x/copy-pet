# Handoff — macOS clipboard flyout: caret anchor & paste-back still broken

- Date: 2026-06-17 · Area: portable backend, macOS (`#[cfg(target_os = "macos")]`)
- Branch with all the code discussed here: `claude/jolly-allen-cj4tr2`
- Status: **unresolved** — both user-facing symptoms persist; another session
  will retry. This doc is the handoff.

## 1. The two problems (user-facing)

When the global hotkey (Cmd+Shift+V) opens the clipboard flyout on macOS:

1. **Wrong position.** The flyout opens at the **mouse cursor**, not at the
   **text caret** of the app being typed in. (Windows opens it at the caret.)
2. **Paste-back fails.** Picking a clip does **not** paste into the field that
   was focused when the hotkey fired. (Windows pastes it at the original caret.)

Goal: macOS should match the Windows Win+V experience — open at the caret, pick,
and have the text land where you were typing, with the cat never moving.

> ⚠️ Contradiction to resolve: in the prior session the user appeared to confirm
> that "the window opens at the caret" worked after the first fix, then reported
> that **neither** works "at all." See the TCC hypothesis in §5 — an unsigned
> `cargo run` binary can have its Accessibility grant silently revoked on the
> next rebuild, which would make a feature "work once, then stop." The next
> session should treat *both* as currently broken and re-confirm from scratch.

## 2. Intended behavior (the Windows reference)

Traced from `src/platform/windows.rs` (works there today):

1. Hotkey fires. **Before stealing focus**, capture (a) the foreground window =
   paste target, and (b) the caret screen position
   (`GetGUIThreadInfo`→`rcCaret`→`ClientToScreen`; mouse fallback if no caret).
2. Open the panel as a **separate window at the caret**; nudge it onto the work
   area. The cat never moves.
3. Pick a clip → hide the flyout → set clipboard → restore the captured target
   (`SetForegroundWindow`+`AttachThreadInput`+`SetFocus`) → `SendInput` Ctrl+V →
   the paste lands at the original caret.

ADR-0013 (`.context/kb/adr/0013-flyout-panel-caret-anchor.md`) is the design of
record; it specifies the macOS equivalents (AX caret read + NSWorkspace/
NSRunningApplication activate + synthesized Cmd+V).

## 3. What was attempted (chronological)

All of this is **already merged into `claude/jolly-allen-cj4tr2`**. The branch
`claude/determined-planck-pnr5p6` from the original task was PR #25 and no longer
exists as a separate ref — it was merged (`38a2efa`), then fixed twice on top:

| # | Commit | What it did | Outcome |
|---|--------|-------------|---------|
| 1 | `16a8253` feat(macos) | Original PR #25 macOS flyout: second winit window, AX caret read (`mac_caret.rs`), mouse fallback | Opened at mouse; no paste-back |
| 2 | `f8b4f23` fix(macos) | Caret: handle a **zero-length (caret) selection** by widening to a 1-char range; added `AXIsProcessTrusted()` gate + `CLIPCAT_DEBUG_CARET` logging. Paste-back: new `mac_focus.rs` (capture frontmost pid before focus steal; re-activate before paste) | Reported still wrong |
| 3 | `965b70a` fix(macos) | Paste keystroke: replaced rdev Cmd+V with native `core_graphics` `paste_cmd_v()` that sets `CGEventFlagCommand` **on the V event**; replaced a 60 ms main-thread `sleep` with a deferred `pending_paste` fired from the winit loop. Recorded LNR-0007 | Reported still broken (both) |

Reasoning behind 2 & 3 (still valid, just not sufficient):
- Empty-range `AXBoundsForRange` often fails for a bare caret → widen to 1 char.
- rdev's macOS path leaves the Command flag off the `V` CGEvent → unreliable.
- Sleeping the main thread blocks the run-loop turn that `activateWithOptions:`
  needs, so the target was never frontmost when the key was posted (LNR-0007).

## 4. Current code state (precise map)

**Caret anchor — `src/platform/mac_caret.rs`:**
- `caret_screen_pos()` (~:96) = `caret_via_ax().or_else(mouse_location)`.
- `caret_via_ax()` (~:100): **returns `None` → mouse fallback** if any of:
  `!AXIsProcessTrusted()` (:101); attribute-name creation fails;
  `AXFocusedUIElement` missing (:150); `AXSelectedTextRange` missing (:146);
  `AXBoundsForRange` fails on both the original and the widened range (:137-142).
- `widen_empty_range()` (~:201) turns a length-0 caret range into a 1-char range.
- `bounds_origin()` (~:163) anchors the caret's bottom-left and scales AX points
  → physical px via `main_scale()` (~:235, primary screen `backingScaleFactor`).
  AX is top-left origin already (no Y-flip); `mouse_location()` (~:250) **does**
  flip (NSEvent is bottom-left origin). Single-monitor Retina math looks correct.
- Debug: `CLIPCAT_DEBUG_CARET` env var logs which step failed (geometry only).

**Flyout open ordering — `src/platform/portable.rs` `open_flyout()` (~:613):**
- :618-622 capture `paste_target_pid = frontmost_app_pid()` (excluding our own
  pid) **before** any focus steal. ✅
- :654 read caret + `place_flyout(...)`; :656 `set_visible(true)`; :657
  `focus_window()`. **The caret is read before the flyout is shown**, so a
  focus-timing bug is *not* the cause (the target's text field is still the
  system-wide focused element at read time). ✅ `place_flyout()` (~:680) places
  the window so the card lands at the anchor, then `fit_delta` slides it on-screen.

**Paste-back — `src/platform/portable.rs` + `mac_focus.rs`:**
- `set_clipboard()` (~:416): set suppress marker → `arboard` set_text → if
  `pick.paste`: `paste_target_pid.take()` → `mac_focus::activate_app(pid)` →
  `pending_paste = Some(Instant::now() + 80ms)` and **return** (no inline paste);
  if no target, `paste_cmd_v()` immediately.
- `after_flyout_action()` (~:874) hides the flyout (`set_visible(false)`) **then**
  calls `set_clipboard()`.
- `about_to_wait()` (~:1117) fires `paste_cmd_v()` once `now >= pending_paste`,
  outside the 33 ms TICK gate; control-flow wake = `min(last_frame+TICK, due)`.
- `paste_cmd_v()` (~:1339): `CGEventSource::new(HIDSystemState)`, post V key-down
  & key-up, each `set_flags(CGEventFlagCommand)`, `post(HID)`.
- `mac_focus.rs`: `frontmost_app_pid()` via `NSWorkspace.frontmostApplication`;
  `activate_app(pid)` via `NSRunningApplication
  runningApplicationWithProcessIdentifier:` + `activateWithOptions:`
  **`NSApplicationActivateIgnoringOtherApps`** (`1<<1`).

**Net:** the code on disk implements the ADR. Static review finds no obvious
logic bug — which is why the persistent total failure points at the *runtime
environment* (permissions / activation semantics) rather than the algorithm.

## 5. Why it is probably still failing — ranked hypotheses

**H1 (leading) — the running binary lacks the macOS *Accessibility* TCC grant,
which is a different bucket than the one the hotkey tap uses.**
- On macOS 10.15+, a **listen-only** `CGEventTap` (our `mac_input.rs`,
  `kCGEventTapOptionListenOnly`) is permitted by **Input Monitoring**, whereas
  **reading another app's AX** *and* **posting synthetic events into another app
  via `CGEventPost`** both require **Accessibility** (`AXIsProcessTrusted`).
- This single cause explains *all* observations at once: the **hotkey still
  opens the panel** (Input Monitoring OK) while **caret read returns `None` →
  mouse** (Accessibility denied → `AXIsProcessTrusted()==false`, :101) **and the
  Cmd+V is dropped** (`CGEventPost` from an untrusted process doesn't reach other
  apps). It also explains "worked once, then stopped": an unsigned `cargo run`
  binary's cdhash changes every rebuild, so macOS silently invalidates its
  Accessibility grant while Input Monitoring may persist.
- ADR-0013 line 47 bakes in the wrong assumption: *"macOS already requires
  Accessibility permission for the event tap, so no new prompt."* The tap likely
  runs under Input Monitoring, so that assumption — and the "no prompt" decision —
  is the probable root cause.

**H2 — `activateWithOptions:` with `NSApplicationActivateIgnoringOtherApps` no
longer reliably brings the target forward (macOS 14+).** Apple deprecated that
option and changed cooperative activation; even with Accessibility granted the
target may not become key in time, so a valid Cmd+V lands nowhere. (Explains
Problem 2 only — cannot explain Problem 1 — so it is secondary to H1.)

**H3 — 80 ms is too short** for activation + responder-chain install before the
deferred Cmd+V (Problem 2 only).

**H4 — the user is running an older build** that predates `f8b4f23`/`965b70a`
(e.g. a stale `target/` binary, or built a different checkout). Cheap to rule out.

**Ruled out:** focus-timing in `open_flyout` (caret is read before the window is
shown, §4); missing Command flag (fixed in `965b70a`); main-thread sleep (removed
in `965b70a`); zero-length caret range (handled by `widen_empty_range`).

## 6. Recommended approach for the next session (diagnose before coding)

The code already matches the design, so **do not rewrite it blind** — instrument
and confirm which hypothesis holds:

1. **Confirm the build.** `git -C copy-pet log --oneline -3` → expect `965b70a`
   at/near HEAD; `cargo build` fresh; run that exact binary. (Rules out H4.)
2. **Run with `CLIPCAT_DEBUG_CARET=1`** and press the hotkey in TextEdit/Notes.
   - If it logs *"not Accessibility-trusted"* → **H1 confirmed.**
   - If it logs *"AX caret unavailable"* after a successful focused-element read →
     an AX-shape/coords issue; capture the raw rect next.
3. **Check System Settings → Privacy & Security**: is the *exact* running binary
   listed and enabled under **Accessibility** (not just **Input Monitoring**)?
   Toggle it off/on after each rebuild; note whether granting Accessibility makes
   *both* caret and paste start working (would confirm H1 decisively).
4. **Isolate the paste** from activation: with Accessibility granted, manually
   click into TextEdit after opening the flyout, then pick a clip — if it pastes,
   the keystroke is fine and the gap is activation (H2/H3).

## 7. Candidate fixes, contingent on the diagnosis

- **If H1:** stop assuming the tap's permission covers AX. Add an explicit
  Accessibility check/prompt at startup using
  `AXIsProcessTrustedWithOptions({kAXTrustedCheckOptionPrompt: true})`, surface a
  clear "grant Accessibility to ClipCat" message (reuse the existing
  `notify_accessibility_needed` path), and document that **release builds must be
  a signed, stable `.app`** so the TCC grant survives updates. Revisit ADR-0013
  line 47. (Dev workaround: grant Accessibility to the terminal running
  `cargo run`, or to a stable-path signed build.)
- **If H2:** replace/augment activation — activate the `NSRunningApplication`
  *and* its window, or post the Cmd+V to the target via a PSN/`CGEventPostToPid`
  variant so delivery doesn't depend on global frontmost state.
- **If H3:** increase the defer (80→120–150 ms) and/or re-post once after a
  second run-loop turn; verify it doesn't reintroduce a race.

Keep every change `#[cfg(target_os = "macos")]`; Windows/Linux/core untouched.

## 8. Open questions for the user

- Which macOS version, and is ClipCat run via `cargo run`/`cargo run --release`,
  or a packaged `.app`? (Decides H1/TCC and H2 deprecation relevance.)
- Earlier the caret seemed to work once — in which app, and did anything (a
  rebuild, a permission reset) change between "worked" and "broken"?
- Is the cat's tap/click counter incrementing while typing? (If yes, the tap is
  live; if AX still fails, that is strong evidence for the Input-Monitoring-vs-
  Accessibility split in H1.)

## 9. References

- ADR-0013 `.context/kb/adr/0013-flyout-panel-caret-anchor.md` (design; see the
  line-47 permission assumption).
- LNR-0006 `.context/kb/lnr/0006-auto-paste-foreground-capture-order.md`
  (Windows paste-target capture order).
- LNR-0007 `.context/kb/lnr/0007-macos-paste-synthesis.md` (Cmd+V flag + the
  main-thread-sleep trap).
- Code: `src/platform/mac_caret.rs`, `src/platform/mac_focus.rs`,
  `src/platform/portable.rs` (`open_flyout`, `place_flyout`, `set_clipboard`,
  `after_flyout_action`, `about_to_wait`, `paste_cmd_v`), `src/platform/windows.rs`
  (the working reference), `src/platform/mac_input.rs` (the listen-only tap).
