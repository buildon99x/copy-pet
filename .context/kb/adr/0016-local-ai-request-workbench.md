# 0016 — Explicit local AI request workbench

- Status: Accepted (2026-10-19)
- Extends: [0013](0013-flyout-panel-caret-anchor.md), [0015](0015-clipboard-tags-and-windows-images.md)

## Context

Preparing a useful AI request repeatedly requires collecting clips, assigning
context, filling reusable instructions and checking that copied credentials do
not travel with the request. The existing desktop panel must expose this flow
on Windows without introducing a provider service or another UI framework.

## Decision

- Add a shared `ai_workbench` core and a workbench mode of the existing panel.
  Footer, Ctrl/Cmd+B, shared context menu and the hand-built Windows tray expose
  it. Existing backend input/clipboard/suppression paths carry confirmed copies.
- Projects are switched explicitly using the project row or P button. Never
  infer them from window titles, global typing or application focus. Each owns
  request text, text-only clip snapshots, section assignments/order, default
  snapshot tags, named templates, literal variable values and unfinished editor.
  A separate atomic `requests.json` preserves old state/history compatibility.
- Snapshots outlive history eviction. Image snapshots retain only an attachment
  reminder, not pixels or a hidden binary/text encoding. Original images must
  be attached separately by the user. Tags merge with original-first priority,
  normalized and bounded by the existing tag contract.
- Templates retain their original body. Fill values in a separate map and
  expand only occurrences in that body; never recursively interpret inserted
  JSON or code. Enforce expanded size before accepting a value. Template
  replacement checks the resulting project budget before changing saved data.
- Bound 24 projects, 40 clips and 24 favorites per project; 32 KiB per draft,
  request, template or expanded request; 256 KiB of string payload per project.
  These combined limits fit below the 64 MiB read ceiling even with worst-case
  JSON escaping. Load validates nested fields; refused changes retain originals.
  Save failures preserve dirty state and show a retry notice.
- A local, dependency-free heuristic scanner replaces recognized token prefixes,
  Bearer values, common credential assignments and PEM private keys. Review shows
  kind/ordinal and a complete copy preview using measured font-width wrapping.
  It never logs source values. Confirmation produces plain text with `paste=false`
  in both flyout and embedded panels. Esc returns without changing originals.
  This scanner applies to workbench output; ordinary history keeps the existing
  rich-format/paste contract. No scanner claims complete secret detection.
- Reuse end-editing, select-all, explicit focused Ctrl/Cmd+V and multiline entry
  rather than adding a framework/editor dependency. Project switching preserves
  an unfinished editor. The backend accepts paste only while that editor's panel
  is actually open. Full cursor editing is outside this MVP.

## Consequences and verification

All data stays local and plaintext; redaction protects only the confirmed copy.
Autosave can lose up to its existing interval on an abnormal exit. Snapshot
storage is deliberately small; a project with large clips can reach the combined
budget before its item count. Existing backend focus restoration and suppression
are reused, with no additional network or global-input data collection.

Tests drive public panel APIs for collection, editing, variables, projects,
confirmation and closed-panel paste refusal; unit tests cover persistence models,
limits, literal expansion, UTF-8 redaction, original preservation, tag merging,
image guidance, and complete wrapping at minimum panel width. Headless PNGs cover
English/Korean home, clips, editing and review. Linux runtime and cross-target
checks cannot replace actual Windows hotkey/tray/clipboard execution.
