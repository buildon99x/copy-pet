# ClipCat Brand Icons Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for inline implementation.

**Goal:** Produce and wire branded Windows and macOS app icons.
**Architecture:** Selected C foreground; tiny-skia exports transparent Windows and tiled macOS surfaces.
**Tech Stack:** Existing Rust/tiny-skia, Python standard library, Apple iconutil.
**Spec:** ../../specs/brand-icons.md

Constraints: preserve clipboard behavior and cat art; no new runtime dependencies;
no Store certification cancellation or additional release; use existing checkout.
Review focus: 16px visibility, light/dark backgrounds, alpha padding, complete
ICO/ICNS sizes, and bundle resources included before codesigning.

- [x] Implement shared brand mark, size-aware rendering and OS surfaces.
- [x] Extend gen_icon to export ICO, iconset, masters and review sheet.
- [x] Produce ICNS and wire it into signed macOS packaging; update Store assets.
- [x] Inspect all size previews; check formats and bundle wiring with focused tests.
- [x] Run release build, Clippy and existing tests; generate regular UI previews.
- [ ] Review, commit/push, and check Windows/macOS CI. Deliver files and limitations.

## Decisions and evidence

- The user rejected the first vector clipboard-cat draft and selected C from the
  generated A/B/C board. C is a Ragdoll in a terracotta document pocket.
- Original bytes and exact pre-call inputs are preserved. Art verification passed:
  69 PNGs, 5 recorded prompts, 31 historical prompts explicitly unrecovered.
- Runtime embeds one 256px derivative; the full original is not bundled/cached.
- Artifact tests failed before ICO/ICNS resources and packaging wiring existed,
  then all3 passed. Default release build, Clippy and 218 Rust tests passed.
- iconutil requires normal macOS permissions in this environment; sandbox failure
  did not indicate an invalid iconset. Normal conversion and ICNS parsing passed.
- Universal macOS app packaging and codesign verification passed.
- Independent read-only review found no blockers; Windows shell execution remains
  separate from PNG/icon-container inspection.
- No release tag or Store certification change is authorized by this icon task.
