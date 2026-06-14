# Handoff — F1–F4: Rich Paste, Context Menu, Pin-Right, Tooltips

**Date:** 2026-06-14  
**Branch:** `claude/laughing-pasteur-td1kjc`  
**Status:** Implementation complete, all tests green, ready to PR/merge

---

## What was done

### F1 — Rich clipboard format preservation

Clips now capture and replay HTML and RTF alongside the plain text:

- **`clipboard.rs`**: `RichFormats { html: Option<String>, rtf_b64: Option<String> }` added to `Clip`.  Inline `b64_encode`/`b64_decode` helpers (no new crate).  `add_copy` takes an optional `formats` arg.  Formats > `MAX_RICH` (1 MiB) are silently dropped.
- **`pet.rs`**: `ClipPick` gained `formats: Option<RichFormats>` and `plain_only: bool`.  `on_copy` takes a 4th `formats` arg.  `paste_plain_text()` getter and `toggle_paste_plain_text()` method with toast.
- **`state.rs`**: `paste_plain_text: bool` added to `Persist` (default `false`).
- **`menu.rs`**: `TogglePastePlainText` menu action.
- **`platform/windows.rs`**: `read_clipboard_rich` reads CF_UNICODETEXT + CF_HTML + RTF in one `OpenClipboard` call.  `set_clipboard_pick` + `write_clip_bytes` write all three formats back on rich paste.  Format IDs cached in `OnceLock<u32>`.
- **Portable (macOS/Linux)**: plain-text-only for Phase 1 — formats are stored in the model but write stays plain text via arboard.  Phase 2 = NSPasteboard support.

### F2 — Inline context menu on panel rows

A "⋯" trigger on the selected row reveals inline "Paste Plain" + "Delete" buttons:

- **`panel.rs`**: `CTX_ZONE`, `CTX_BTN_W`, `PIN_ZONE_R`, `DEL_ZONE` constants.  `PanelAction::OpenContextMenu(id)`, `PasteAsPlainText(id)`, `CloseContextMenu`.  `NavKey::ContextMenu` (Right-arrow).  `Panel.context_id: Option<u64>`.
- **`pet.rs`**: handlers for `OpenContextMenu`, `PasteAsPlainText`, `CloseContextMenu`, `Delete` (clears `context_id` on delete).
- **`render.rs`**: context menu button rendering on selected row when `context_id` is set.

### F3 — Pin star moved to the right side

The pin ★ button is now on the **right** side of the row (between the "⋯" zone and the delete zone), matching the "⋯" / Delete layout.  Left side of clip rows is now all body text (18 px wider).

### F4 — Tooltips on the 6 header buttons

Dark rounded-rect tooltips appear below each header button (View, Filter, Pause, Clear, Lang, Close) when the cursor hovers over them.  Localized (En/Ko) via `i18n`.

---

## Files changed (summary)

| File | What changed |
|---|---|
| `src/clipboard.rs` | `RichFormats`, `b64_encode/decode`, `MAX_RICH`, `formats` field on `Clip`, `add_copy` 3-arg |
| `src/state.rs` | `paste_plain_text` field on `Persist` |
| `src/menu.rs` | `TogglePastePlainText` action |
| `src/i18n.rs` | 12 new messages (menu, toasts, btn labels, 6 tooltips) |
| `src/panel.rs` | Right-side zone constants, `PanelAction` and `NavKey` variants, `context_id` on `Panel` |
| `src/pet.rs` | `ClipPick` extension, `on_copy` 4-arg, context menu handlers, `paste_plain_text()` / `toggle_paste_plain_text()` |
| `src/render.rs` | `paste_plain_text` on `PanelView`, tooltip rendering, F3 pin-right layout, F2 context buttons |
| `src/platform/windows.rs` | Rich clipboard read/write (`read_clipboard_rich`, `set_clipboard_pick`, format ID caching) |
| `src/platform/portable.rs` | 4th `None` arg on `on_copy` call |
| `examples/preview.rs` | Updated `add_copy` (3rd arg) and `PanelView` (`paste_plain_text`) |
| `tests/e2e.rs` | 4 new tests: `rich_format_stored_and_returned_in_pick`, `rich_format_size_cap`, `context_menu_paste_plain_and_esc_order`, `context_menu_delete_removes_clip` |
| `.context/kb/adr/0013-rich-clipboard-formats.md` | New ADR |
| `.context/kb/adr/README.md` | ADR index updated |

---

## Quality gates

- `cargo test --release` — 18/18 lib+e2e tests pass, 3/3 release-script tests pass
- `cargo clippy --release` — clean (0 warnings)
- `cargo clippy --release --features portable` — clean
- `cargo build --release` — not run (headless Linux, no display for linkage)
- Preview PNGs — not run (same environment constraint); visual review in CI

---

## What is NOT done / deferred

- **macOS NSPasteboard** (F1 Phase 2): arboard doesn't expose HTML/RTF.  Needs `mac_clipboard.rs` with Objective-C NSPasteboard read/write, similar to `mac_present.rs`.
- **Linux X11 / Wayland rich clipboard**: not planned; plain-text-only is the right scope for Phase 1.
- **CHANGELOG.md**: not updated — per policy, add bullets under `[Unreleased]` before releasing: rich format paste, context menu, pin position, tooltips.

---

## Appendix — Context menu zone layout

```
row: |  ← body text →  |⋯|★| del |
      row_x          row_x+row_w
                   ← CTX_ZONE (20) ─┘
             ← PIN_ZONE_R (24) ──────┘
       ← DEL_ZONE (28) ────────────────┘

When context menu is open:
|  ← body text →  | [Paste Plain] | [Delete] |
                   ← CTX_BTN_W (56) ─┘        |
                                 ← DEL_ZONE ──┘
```
