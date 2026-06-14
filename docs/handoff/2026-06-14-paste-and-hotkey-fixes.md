# Handoff — auto-paste focus fix (F1) & panel-hotkey fallback transparency (F5)

- **Date:** 2026-06-14
- **Branch:** `claude/kind-maxwell-a78zu8` (merged to `main`)
- **Context:** post-PR #21 follow-up. Three issues were reported in real-world
  testing of the Tier-1 convenience work; this session shipped the two bug
  fixes (F1, F5) and deferred the feature work (F2) to a separate task whose
  design is preserved in the appendix below.

## Shipped this session

### F1 — auto-paste now lands in the target app (Windows)

- **Symptom:** with "Paste on select" enabled, picking a clip copied it but did
  not paste into the previously-focused app.
- **Root cause:** `App::copy_back` in `src/platform/windows.rs` called
  `SetForegroundWindow(target)` (asynchronous, and unable to promote a window
  owned by another thread on demand) and *immediately* synthesized Ctrl+V with
  `SendInput`. The keystroke fired before focus actually reached the target,
  so it landed on our own window / nowhere. Classic
  `SetForegroundWindow` + `SendInput` race across a thread-input boundary.
- **Fix:** attach our input queue to the target's thread with
  `AttachThreadInput` around the foreground switch, so the foreground/focus
  state is synchronized before the synthesized paste, then detach. Zero new
  dependencies — added imports `AttachThreadInput`, `GetCurrentThreadId` (both
  live in `Win32::System::Threading`, **not** `KeyboardAndMouse`).
- **Note:** the auto-paste feature itself is still `[Unreleased]`, so no
  separate CHANGELOG "Fixed" bullet was added — its existing `Added` entry now
  simply describes working behavior.

### F5 — panel hotkey no longer silently mismatches the saved setting (Windows)

- **Symptom:** `state.json` held `win+shift+v` but the panel opened on
  `ctrl+shift+v`, and the menu/hint label showed `ctrl+shift+v` — a silent
  mismatch with the saved setting.
- **Root cause:** `register_panel_hotkey` fell back from the configured chord to
  the `Ctrl+Shift+V` fallback **silently** when `RegisterHotKey` failed.
  Win+Shift+V is reserved by Windows' own clipboard-history feature, so the
  configured chord cannot be registered while that is on.
- **Fix:** `register_panel_hotkey` now returns a `HotkeyReg` enum
  (`Configured` / `Fallback { wanted, used }` / `None`) instead of a bare label.
  The backend calls the new `Pet::notify_hotkey_fallback(wanted, used)` at
  startup and on menu-cycle, which toasts a localized explanation
  (`i18n::hotkey_fallback`, EN + KO). The configured chord stays saved, so it
  registers normally once whatever holds it is freed (e.g. the user turns off
  Windows clipboard history).
- **CHANGELOG:** one `Fixed` bullet added — this silent fallback existed in
  shipped versions, so it is user-facing.

### Files touched
`src/platform/windows.rs`, `src/pet.rs`, `src/i18n.rs`, `CHANGELOG.md`
(+ this handoff). Commit: `ac5b496`.

## Verification

- `cargo test --release` — pass (14 e2e + i18n completeness incl. the new
  `hotkey_fallback` assertion + release-script tests).
- `cargo clippy --release --target x86_64-pc-windows-msvc` — clean (the backend
  carrying both fixes).
- `cargo clippy --release` (portable) and
  `cargo check --release --target aarch64-apple-darwin` — clean.
- `cargo run --release --example preview` — renders; `scripts/release.sh verify`
  — CHANGELOG OK.
- **Limitations (honest):** real key injection and OS hotkey reservation cannot
  be reproduced on a headless box. The F1 paste and the F5 fallback toast must
  be confirmed on a Windows runtime / via CI builds. The fixes follow the
  documented Win32 remedies, so confidence is high, but a Windows smoke test
  (Notepad focus → pick a clip → text lands; clipboard-history on → fallback
  toast appears) is the final gate.

## Follow-ups / open work

- **F2 — original-format preservation + plain-text paste option** is a
  **separate task, not started.** User decision: default = preserve original
  formatting, plain-text = an option, exposed as a single global toggle
  (platform-agnostic). Full design is in the appendix below (it was also kept in
  the ephemeral planning dir at `~/.claude/plans/f2-rich-clipboard-formats.md`,
  which does not survive container recycling — the appendix is the durable
  copy).
- Windows smoke test of F1/F5 (see Limitations).

---

## Appendix — F2 design (deferred, not started)

> Preserved here so the separate-task design is not lost. Start from this when
> F2 is picked up; pair it with an ADR-0005 revision.

**User decision (confirmed):** default = preserve original formatting;
plain-text paste = option; exposure = a single global toggle (one menu item,
platform-agnostic).

**Reality / constraints**
- macOS uses `objc` 0.2 (no native clipboard code today; shares arboard).
- **Linux rich formats are unsupported by arboard** → would need a new
  x11rb/wl-clipboard dependency + X11 selection protocol work (conflicts with
  golden rule #3 "no heavy deps"; Wayland gap). → phased, Linux rich formats
  last and ADR-gated.

**Data model (`src/clipboard.rs`)** — optional sidecar on `Clip`, opaque to the
core (backends encode/decode):
```rust
#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct RichFormats {
    #[serde(default, skip_serializing_if = "Option::is_none")] pub html: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub rtf_b64: Option<String>,
}
// Clip { ..., #[serde(default, skip_serializing_if="Option::is_none")] pub formats: Option<RichFormats> }
```
- Search/preview keep using `Clip.text` only (unchanged); formats are not
  searched.
- serde back-compat: old `clips.json` has no `formats` → None; plain clips omit
  it on serialize.
- RTF as **base64 String** (avoid serde_json's `Vec<u8>` = integer-array bloat);
  HTML as UTF-8 String. base64 via a **~30-line inline helper** (no new crate).
- Separate cap `MAX_RICH = 1 MiB`, independent of text's `MAX_TEXT = 256 KB`.
  On overflow **drop only `formats`, keep the plain clip** (graceful). On a
  duplicate `add_copy` bump, refresh `formats` like `source`.

**Capture**
- **Windows** (`windows.rs`, 0 new deps): cache atoms via
  `RegisterClipboardFormatW("HTML Format")` / `("Rich Text Format")` at startup;
  generalize `read_clipboard_text` → `read_clipboard_rich`: one OpenClipboard
  reads CF_UNICODETEXT (required) + CF_HTML + CF_RTF. **Store/restore the CF_HTML
  blob verbatim incl. its header** (avoid offset recomputation). Deliver via
  WM_CLIPBOARDUPDATE → on_copy.
- **macOS** (new `src/platform/mac_clipboard.rs`, `objc` 0.2, 0 new deps): track
  NSPasteboard `changeCount` (a real change counter → eases part of ADR-0005's
  polling limit); `stringForType:` + `dataForType: public.html / public.rtf`.
  Reuse the `msg_send!` pattern from `mac_dialogs.rs` / `mac_menu.rs`, wrap in an
  autorelease pool. Portable watcher calls this module on macOS.
- **Linux:** arboard is text-only → **Phase 1 keeps plain capture** (current);
  rich formats are Phase 3.

**Restore**
- Toggle OFF (default) → write all captured formats; ON → plain text only.
- **Windows** `copy_back`: `set_clipboard_text` → `set_clipboard_rich(text,
  formats, plain_only)`: one OpenClipboard always sets CF_UNICODETEXT, plus
  CF_HTML/CF_RTF when `!plain_only`. **The self-suppression marker is keyed on
  text, so it stays valid** (text is always written).
- **macOS** `mac_clipboard` write: `clearContents` + `setData:forType:` per type.
  Suppress (text) unchanged.
- **portable/Linux** `set_clipboard`: Phase 1 plain text (off + Linux degrades
  to plain).

**Contract / state / wiring**
- `ClipPick { text, paste, formats: Option<RichFormats>, plain_only: bool }`;
  `run_action` Copy fills `c.formats.clone()` + `plain_only:
  st.paste_plain_text`. Core only forwards opaquely.
- `Persist.paste_plain_text: bool` (default false = preserve, `#[serde(default)]`);
  add to old-config tests.
- Extend `add_copy(text, source, formats)` / `Pet::on_copy(.., formats)`
  (callers: Windows, portable).
- Portable watcher channel `Sender<String>` → `ClipCapture { text, formats }`
  (Linux: formats = None).
- Menu: `MenuAction::TogglePastePlainText` + `Pet::toggle_paste_plain_text`
  (clone of `toggle_paste_on_select`) + `build_menu` leaf (after Paste-on-select)
  + `apply_menu_action` arm + Windows tray `CMD_PASTE_PLAIN`.
- i18n: `MenuPastePlainText` / `ToastPlainOn` / `ToastPlainOff` + completeness
  test.

**Orthogonal to `paste_on_select`:** `paste_on_select` = whether to auto-send
Ctrl/Cmd+V (`ClipPick.paste`); `paste_plain_text` = what to put on the clipboard
(`ClipPick.plain_only`). Different fields/flags → all 4 combinations work
naturally.

**Deps & ADR:** new crates: Windows 0, macOS 0 (reuse objc 0.2), base64 inline.
**Only Linux rich formats need a new dep (x11rb/wl-clipboard) → ADR required,
excluded from Phase 1.** Supersede ADR-0005 → `0013-rich-clipboard-formats.md`
(text + optional HTML/RTF; images/files remain non-goals; record data
model/MAX_RICH/back-compat/platform asymmetry/privacy/orthogonality).

**Tests:** core (headless) — serde back-compat, RichFormats round-trip, plain
clips omit the key, `add_copy` store/refresh/MAX_RICH drop, base64 round-trip,
`ClipPick` intent, toggle/menu/`apply_menu_action`, i18n completeness, the 4
orthogonal combos. Platform side-effects (manual/CI) — real CF_HTML/RTF and
NSPasteboard read/write, multi-format single session, suppression with formats.

**Phases**
- **Phase 1** (core + toggle, all platforms): data model, base64, MAX_RICH,
  ClipPick/add_copy/on_copy, Persist, menu/i18n, channel, core tests, the
  superseding ADR. Restore respects `plain_only` (still writes plain only). →
  the "paste as plain text" toggle works everywhere immediately.
- **Phase 2** (Windows + macOS rich): `read`/`set_clipboard_rich`,
  `mac_clipboard.rs`. Manual verification.
- **Phase 3** (Linux rich, optional, ADR-gated): x11rb selections (text/html) +
  document the Wayland gap; only if the new dep is justified.

**Risks:** (1) Linux rich difficulty / new dep (highest) → phased out.
(2) CF_HTML header/offset → re-emit the blob verbatim. (3) macOS objc 0.2 unsafe
→ reuse existing pattern + autorelease. (4) clips.json bloat →
skip_serializing / MAX_RICH / base64. (5) suppression invariant → always write
text (note inline).
