# ADR-0013 — Rich clipboard format preservation (HTML + RTF)

**Status:** Accepted  
**Date:** 2026-06-14

## Context

ClipCat captured and replayed plain `CF_UNICODETEXT` only (ADR-0005). Users who
copy from apps like VS Code, a browser, or Word lose all formatting — bold, color,
code syntax — when they replay from the history. The ask is to replicate the
behavior of Windows Win+V: rich format paste by default, with a "paste as plain
text" escape for when you want just the characters.

Two registered formats carry rich clipboard content:
- **`HTML Format`** – a registered clipboard format; UTF-8 bytes with a header
  (version line, byte offsets) then the HTML fragment. Present in most modern
  apps.
- **`Rich Text Format`** – raw RTF bytes (binary-safe). Present in word
  processors and some editors.

## Decision

**Core model:** `RichFormats { html: Option<String>, rtf_b64: Option<String> }`
added to `Clip`. Both fields are nullable and `skip_serializing_if = "Option::is_none"`
so `clips.json` stays backwards-compatible with plain clips. Formats exceeding
`MAX_RICH = 1 MiB` combined are silently dropped to prevent runaway clipboard
content from bloating the history file.

**RTF base64:** RTF bytes are not valid UTF-8 in general. Rather than add a
`base64` crate, the project includes two ~60-line inline helpers `b64_encode` /
`b64_decode` (standard Base64, no padding). The choice preserves the
no-new-dependency goal (Golden Rule 3).

**Windows:** the native backend reads `HTML Format` + `Rich Text Format` in the
same `OpenClipboard` call as `CF_UNICODETEXT` (new `read_clipboard_rich`
function). On `copy_back`, when `plain_only` is false and formats are present,
it writes all three in one `OpenClipboard / EmptyClipboard / ... / CloseClipboard`
round (new `set_clipboard_pick` / `write_clip_bytes` helpers). Format IDs are
looked up once via `RegisterClipboardFormatW` and cached in `OnceLock<u32>`.

**Portable / macOS / Linux (Phase 1 — plain text only):** arboard does not
expose rich clipboard types. Reading and writing stay plain text on the portable
backend. Formats arriving via `on_copy` are stored in the model and will be
usable when macOS NSPasteboard support is added (Phase 2). No new dependency
was introduced for Phase 1.

**User-visible toggle:** `paste_plain_text: bool` in `Persist` (default
`false`). Menu item `TogglePastePlainText` flips it with a toast. When true, all
`ClipPick`s carry `plain_only = true` regardless of stored formats. The inline
context menu's "Paste Plain" action (`PasteAsPlainText`) always produces
`plain_only = true` on a per-clip basis, ignoring the global flag.

## Consequences

- `clips.json` is forward- and backwards-compatible: old app versions ignore
  unknown keys; new app with old file reads `formats: null` (handled by
  `#[serde(default)]`).
- The clipboard open/close on `WM_CLIPBOARDUPDATE` now reads three formats
  instead of one; it remains a single OS call.
- `MAX_RICH = 1 MiB` is a conservative cap; a typical HTML Format payload is
  < 100 KiB.
- macOS rich paste is deferred. macOS users get stored formats in their
  `clips.json` if they ever run both platforms, but paste is plain-text-only
  until Phase 2.
- No new crates were added. The b64 helpers add ~70 source lines.
