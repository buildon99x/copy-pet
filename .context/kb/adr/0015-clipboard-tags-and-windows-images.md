# 0015 — Clipboard tags and Windows image history

- Status: Accepted (2026-10-19)
- Extends: [0014](0014-rich-clipboard-formats.md), [0013](0013-flyout-panel-caret-anchor.md)

## Context

Users need to organize clips independently of source applications and reuse
images copied on Windows. Text-only capture discards screenshots entirely.
The existing history, panel, and pick contract should continue to work with
old saved data, without adding an image framework or another network feature.

## Decision

- Extend `Clip` with optional `image` and default-empty `tags`. Text clips keep
  their text and HTML/RTF behavior. Image clips carry PNG data, dimensions, and
  an empty text field; image identity is separate from text identity.
- Reuse tiny-skia's existing PNG codec. `clip_image.rs` validates PNG dimensions
  before decoding, converts uncompressed 24/32-bit Windows DIBs (including
  32-bit bitfields), and produces a cached, bounded thumbnail for the panel.
- Windows native capture prefers registered PNG, then CF_DIBV5 and CF_DIB,
  then existing text formats. Emit PNG plus an opaque 24-bit CF_DIB fallback
  when picking an image. PNG preserves alpha; the DIB fallback composites it
  on white. Do not emit a textual image label onto the clipboard.
- Limit decoded images to 16,777,216 pixels, PNG data to 16 MiB, incoming DIB
  allocations to 64 MiB plus header allowance, and encoded image history plus
  session undo payloads to 64 MiB. Refuse a new image that exceeds the budget;
  do not evict pinned data to make it fit. Validate images on history load.
- Store PNG as base64 in the existing atomic JSON history. This avoids orphan
  image files and lets delete/undo retain the same ownership model. A larger
  binary sidecar store can be considered separately if the bounded JSON proves
  too costly. No new dependency or network destination is introduced.
- Windows self-copy suppression uses clipboard sequence numbers for both
  text and images. A later external copy of identical content is still a copy.
  Keep the existing focus restoration and paste-before-hide ordering.
- Tags use commas/whitespace as separators, optional leading `#`, at most eight
  tags of 24 Unicode characters each, and case-insensitive deduplication. Empty
  input removes tags. Normal search includes tags; `#tag` matches an exact tag
  and combines with other query terms and the existing source filter.
- Tag editing reuses the panel search field, entered through the row's `#`
  action or Ctrl/Cmd+T. Enter saves, Esc cancels, and Ctrl/Cmd+A selects the
  draft. Editing never copies a clip or closes the panel. Changes participate
  in the existing autosave/shutdown persistence and filtered-view invalidation.

## Consequences and validation

Old text histories load without migration. Pins, tags, deletion, undo, quick
copy, and flyout paste intent apply to image clips too. Paste-as-text is not
available for an image. Windows portable, macOS, and Linux image clipboard I/O
remain outside this implementation; tags and image rendering are shared core.

Core tests cover binary validation, row orientation/padding/alpha, persistence,
tag search and editing, and image pick intent. A Windows-only test exercises
actual native clipboard text/PNG/DIB roundtrips with synthetic content. A Linux
cross-target check can compile that test but cannot execute it. Real Windows
hotkey/embedded-panel paste checks remain necessary to validate OS behavior.
