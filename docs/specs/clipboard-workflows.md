# Clipboard workflow improvements

Date: 2026-11-27. Implements the three feature proposals accepted in this chat.

## Intent and constraints

Implement in priority order: capture privacy, ordered paste, searchable image text.
Keep the existing checkout, legacy JSON compatibility, rich formats, image identity,
copy suppression, focus restoration and the quiet cat. No input hooks, activity
statistics, network services, language-model downloads or bundled fonts. Windows
native is the primary product. English/Korean strings belong in i18n.rs.

## 1. Capture privacy

Provide a usable settings/menu path to exclude sources observed in clipboard
history and restore excluded sources. Reject excluded copies before storing them
or triggering cat feedback. Match source names case-insensitively; identify this
as application-level exclusion, not website/password-field detection. Read the
source application's identity at the clipboard event, never titles or contents.
Honor confidential/transient clipboard markers on macOS and supported Windows
clipboard exclusion formats before reading payloads. Linux has no claimed marker
guarantee. Keep existing pause/resume synchronization.

Offer retention presets: unlimited (legacy default), 1, 7 and 30 days. Pinned
items are exempt. Apply expiration on load, setting change and periodically;
expired items must not be resurrected by undo, queued paste or late OCR results.
Expose capture/retention state through localized menus/panel feedback. Existing
stored content is not retroactively purged just by excluding a source; explain
the distinction. Exclusion affects ClipCat only, not OS/other apps' history.

Windows honors `ExcludeClipboardContentFromMonitorProcessing` and a zero
`CanIncludeInClipboardHistory` flag. macOS honors concealed, transient and
auto-generated pasteboard markers. macOS identifies the foreground application
when polling. A rapid app switch or background writer can produce a different
name. X11 uses the selection owner's WM_CLASS when available. Unknown sources
remain eligible. macOS and Windows use clipboard change numbers to acknowledge
a self-write. X11 uses the selection-owner window when the X server can verify
its process ID through XRes. Without that identity a self-write may appear as
a new copy. The portable shortcuts with the panel closed are `]` to cycle
sources, `E` to toggle exclusion and `R` to cycle retention. Seconds since the
last copy determine expiry; periodic cleanup runs once per minute.

## 2. Ordered paste

Select text/history entries into a bounded session-only ordered queue. Provide
visible queue membership/order/count, remove/reorder/reset and a way to replay
the previous queued item. Paste the next item with an OS-registered dedicated
shortcut, without observing/replacing ordinary Ctrl/Cmd+V. Register/unregister
only while needed, report conflicts, and preserve existing panel shortcuts.
Snapshot entries so regular history eviction cannot silently change queued data;
explicit delete, expiry, capture privacy changes and clear operations must not
leave hidden usable copies of removed sensitive clips. Never skip an item on
clipboard-write/injection failure. Keep original formats/images on next paste.
Also combine selected text entries in queue order with newlines for one paste;
never silently discard an image when combining. Focus goes to the current
external target; no automatic Tab/Enter or form submission. Document OS limits.

The queue holds 16 snapshots, including already pasted entries. F3 toggles
the selected history row. F2 opens the queue. Up/Down selects, Left/Right
moves, Delete removes, F4 clears, Enter pastes next, F5 replays the most
recent completed item, and F6 combines the queue as plain text. The dedicated
registered shortcut defaults to Ctrl+Alt+J (Control+Option+J on macOS).
F7 or a click on its label cycles J/K/L presets and persists that choice. Registration
is active while a next item is available or an explicit portable panel
operation awaits its target. Windows native and macOS restore the target
window. Windows portable and X11 let the user focus the destination and
invoke the shortcut to complete a panel-initiated paste. Wayland has no
global queue shortcut. macOS requires Accessibility for posting paste events.
An acknowledged paste means that ClipCat wrote the requested formats and
submitted the OS paste events. Applications can decline the paste. Image
output requires Windows native. Capture pause clears all snapshots, explicit
clear preserves pins, and source exclusion removes matching snapshots.

## 3. Image text

Windows native first: explicit per-image local OCR action, plus an opt-in setting
for new-image background indexing if reliable within the same bounded pipeline.
Once recognized, text is searched by the existing query engine, with the original
thumbnail retained and an explicit copy-text action/preview. OCR metadata must not
replace image identity, rich formats or original bytes. Keep old clips readable.
Use the installed Windows OCR capability; no cloud or model bundle. Bound image
size, work queue, duration and output; keep UI responsive, clean up resources,
show localized empty/error/unavailable results and allow retry. Late results for
removed/expired clips must be discarded. Supported installed OCR languages govern
recognition; Windows execution and Korean/English accuracy need actual OS proof.
Other platforms retain image behavior and must not advertise unavailable OCR.

The native Windows panel uses F8 for recognition/retry, F9 for preview and F10 for plain-text copy. In the preview, Ctrl+C, Ctrl+Insert and the Copy menu copy the selected characters. Selection copies use the same plain-text writer and clipboard notification suppression as F10. Recognition is manual. One active job accepts at most a validated 16-megapixel image, times out after 20 seconds, and returns at most 64 KiB of UTF-8. The helper exchanges data through anonymous pipes. Pause, source-exclusion changes, deletion and expiration invalidate in-flight results. Recopying or restoring the same image also invalidates the previous request. Historical OCR remains subject to the same retention rules as the source image.

Microsoft documents package identity as a requirement for Windows.Media.Ocr. The Store build supplies package identity. Standalone availability depends on the host. A missing activation or profile-language engine produces the localized unavailable message. English and Korean recognition require corresponding installed Windows OCR support and a matching user-profile language. ClipCat does not install language packs.

## Alternatives and choices

Privacy uses source exclusion and storage limits rather than promising universal
password detection. Ordered paste uses registered notifications rather than an
input hook. OCR uses OS capabilities rather than introducing a heavy runtime.
These are implementation choices made under the accepted feature proposals.

## Verification

Meaningful tests exercise synthetic confidential copies, expiration/undo, queue
ordering and failed paste acknowledgment, recognition search/persistence and late
results. Run release build, Clippy and complete existing tests for each feature.
Generate and inspect English/Korean previews for changed UI. Run the macOS app
in an isolated test profile, exercise reachable flows, capture screenshots, then
quit. Verify Windows native/portable compilation and CI when possible; never
equate cross-compilation with real Windows manual testing. Commit/push each unit
over the repository's SSH remote; do not release or promote production.
