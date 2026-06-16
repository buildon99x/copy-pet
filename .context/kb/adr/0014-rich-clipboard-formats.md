# 0014 — Rich clipboard formats (HTML/RTF) preserved by default + per-row "paste as text"

- Status: Accepted (2026-06-16)
- Supersedes: [0005](0005-clipboard-manager.md) (text-only clipboard model)
- Related: [0012](0012-auto-paste.md) (auto-paste via synthesized Ctrl/Cmd+V),
  [0010](0010-movable-resizable-panel.md) (panel layout)

## Context

ClipCat captured and restored **plain text only** (ADR-0005), so copying rich
content (bold, colors, links) and pasting it back lost all formatting — unlike
Windows' own Win+V, which preserves the original formatting and additionally
offers an explicit "paste as plain text". This was the headline parity gap left
after ADR-0012 closed the auto-paste gap.

The design was scoped earlier (the deferred "F2" note in
`docs/handoff/2026-06-14-paste-and-hotkey-fixes.md`). Two constraints shaped it:
the "no heavy dependencies" golden rule, and the platform asymmetry of clipboard
APIs — Windows and macOS expose rich formats with the crates already in the tree,
but the portable backend's `arboard` is text-only and Linux rich formats would
need a new X11/Wayland selection dependency.

## Decision

**Preserve the original formatting by default; expose plain-text paste as a
per-row action** (not a global toggle — that was the original F2 sketch, revised
during implementation).

1. **Data model** (`src/clipboard.rs`): an optional, opaque sidecar on `Clip`:
   ```rust
   pub struct RichFormats { html: Option<String>, rtf_b64: Option<String> }
   Clip { …, #[serde(default, skip_serializing_if="Option::is_none")] formats: Option<RichFormats> }
   ```
   Plain text stays the source of truth for search, preview and de-dupe; formats
   are never searched. RTF bytes are base64 (a ~50-line inline helper, no crate —
   raw `Vec<u8>` bloats serde_json). A separate `MAX_RICH = 1 MiB` cap drops only
   the formats on overflow, keeping the plain clip. `add_copy_rich` /
   `Pet::on_copy_rich` carry them; `add_copy` / `on_copy` are the `None` wrappers.
   serde back-compat: old `clips.json` lacks the key → `None`; plain clips omit it.

2. **Pick contract** (`src/pet.rs`): `ClipPick { text, paste, formats, plain_only }`.
   A normal pick sends `formats` with `plain_only=false` (preserve); the per-row
   **"paste as text"** action (`PanelAction::PasteText`) sends `plain_only=true`
   and `paste=true` (strip + paste explicitly).

3. **Capture / restore**:
   - **Windows** (`platform/windows.rs`, 0 new deps): `RegisterClipboardFormatW`
     atoms for `"HTML Format"` / `"Rich Text Format"`, cached. `read_clipboard_rich`
     reads CF_UNICODETEXT + CF_HTML + CF_RTF in one `OpenClipboard` (CF_HTML kept
     verbatim incl. header). `set_clipboard_rich` always writes CF_UNICODETEXT,
     plus the rich formats when `!plain_only`.
   - **macOS** (new `platform/mac_clipboard.rs`, reuses `objc` 0.2 — 0 new deps):
     `NSPasteboard` `stringForType:`/`dataForType:` for `public.html`/`public.rtf`;
     write via `clearContents` + `setString:`/`setData:forType:`. The portable
     watcher reads it; `set_clipboard` writes it. Only pasteboard data is touched
     (never a TIS API), so it is safe off the main thread (cf. LNR-0005).
   - **Linux**: `arboard` is text-only → formats are always `None`; "paste as
     text" and normal paste both write plain text (graceful degrade).

4. **UI**: the per-row pin moved to the right; an overflow "..." (or the **→**
   key) reveals `[ paste as text ] [ delete ]` for the selected row (**←**/Esc
   collapse). The always-visible delete "X" is folded into that menu.

The self-suppression marker (skip our own copy-back once) stays keyed on the
**text**, which is always written, so it remains valid with or without formats.

## Consequences

- Win+V parity: rich content round-trips by default on Windows and macOS; an
  explicit, discoverable "paste as text" strips formatting per clip.
- No new crates on Windows or macOS; base64 is inline. Linux gains nothing here
  and needs no new dependency.
- `clips.json` can grow with HTML/RTF; bounded by `MAX_RICH` + `skip_serializing`
  for plain clips. Privacy posture is unchanged — clipboard content (now richer)
  still stays local; no network use is added.
- Platform asymmetry is now explicit and documented (Win/mac rich, Linux plain).
  Linux rich formats remain a future, dependency-gated possibility.
- The native CF_HTML/CF_RTF and NSPasteboard read/write paths cannot be verified
  in a headless Linux CI/build; they are covered by cross-target `cargo check`,
  the platform builds, code review, and a manual Windows/macOS smoke test. The
  core (data model, base64, `ClipPick` intent, panel actions) is fully unit/e2e
  tested headless.
