# ADR-0005: Clipboard manager — per-backend watchers, text-only, arboard on portable

- Status: Accepted
- Date: 2026-06-11
- Related: [ADR-0001](0001-cross-platform-architecture.md) (backend split)

## Context

v2.0 makes clipboard management the core feature (ClipCat). We need to
(a) observe every system-wide text copy, (b) write text back to the clipboard
from the history panel, on Windows + macOS + Linux, without compromising the
project's privacy posture (no network, minimal dependencies) or the backend
split (core never touches the OS).

Options considered for observation: a cross-platform clipboard crate
everywhere; native APIs everywhere; or native-on-Windows + crate-on-portable.
Windows has a real change notification (`WM_CLIPBOARDUPDATE` via
`AddClipboardFormatListener`) plus owner-process metadata (app name, icon)
that no cross-platform crate exposes. macOS/Linux have no portable
notification API at all — every clipboard manager polls.

## Decision

1. **Core model is platform-agnostic**: `clipboard::ClipStore` (history,
   pins, search, eviction, `clips.json` persistence) and `panel.rs` know
   nothing about the OS. Backends feed `Pet::on_copy(text, source, badge)`
   and receive `Option<String>` ("put this on the clipboard") from panel
   interactions.
2. **Windows native backend uses raw Win32**: clipboard listener, CF_UNICODETEXT
   read/write, owner-process name + `ExtractIconExW` icon for the fish badge.
   No new dependency.
3. **Portable backend adds `arboard`** (`default-features = false`, so no
   image support and no wayland-data-control stack) in the same
   optional+target-table layout as the other portable crates (LNR-0004).
   A watcher thread polls `get_text()` every ~400 ms.
4. **Text only, size-capped**: clips over 256 KB are ignored (truncation
   would corrupt a later paste); images/files are out of scope (ADR'd
   non-goal, keeps arboard image deps out).
5. **Self-suppression contract**: any backend that writes the clipboard
   records the text in a one-shot marker and skips the next matching change
   event, so copy-backs never spawn fish or re-add clips.

## Consequences

- ✅ Copy events are instant + metadata-rich on the release target (Windows);
  the same core works everywhere else with a small, text-only dependency.
- ✅ Privacy story stays simple: clip content lives only in `clips.json`,
  capture is pausable, no network.
- ⚠️ Portable capture latency is the poll interval (~0.4 s) and consecutive
  identical copies are indistinguishable there (no change counter).
- ⚠️ On pure Wayland (no XWayland) arboard-without-wayland-feature finds no
  clipboard; the watcher exits quietly — same class of limitation as rdev's
  global input (documented in README/spec).
- ⚠️ `arboard` adds ~3 transitive crates on Linux (x11rb stack); accepted as
  the cost of macOS/Linux parity.
