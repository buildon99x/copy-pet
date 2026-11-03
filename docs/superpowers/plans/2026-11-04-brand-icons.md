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
- [x] Review, commit/push, and check Windows/macOS CI. Deliver files and limitations.

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
- Implemented and pushed as 7906303. CI run 41664590099: Windows icon-resource
  validation, build and Clippy passed; tests running at the last recorded check.
  Remote macOS job was queued. This is not a claim of remote CI completion.
- Locally verified the actual universal .app's CFBundleIconFile and matching ICNS
  bytes, codesign integrity, and the icon returned by NSWorkspace for that bundle.
  Exported Finder preview: /private/tmp/clipcat-icon-native/finder-icon.png.
- Native Windows shell display was not manually inspected; the size review is a
  render preview. Existing Store submission remains the previous icon/version.

## Release follow-up (2026-11-04)

- The user subsequently authorized checking CI and releasing the selected C icon
  through Microsoft Store. The earlier icon-only scope above is historical.
- Source CI 41664590099 completed successfully on Windows and macOS.
- Prepared v2.5.4 at 10aa9df8344c07b47d92600b094efe5aa4da7bfc with the
  release script's full local checks. The tag CI's first macOS native paste probe
  failed after Return passed; the same commit's main CI 41666431759 passed all
  three paste probes. Failed tag jobs were rerun without weakening checks.
- Tag CI attempt 2 and Store package CI 41671442148 succeeded. v2.5.4 is
  public and production equals its exact commit. Windows decoded the packaged
  executable's C icon successfully; the MSIX tile hash matches the C export.
- Replaced the v2.5.3 draft package with validated v2.5.4.0 and saved C app
  tiles in both Store languages. Resubmission was accepted: In certification,
  Pre-processing step 2/4 at 2026-11-04 01:24 KST; automatic publish on pass.
  Public Store availability remains pending. See the sanitized receipt at
  ../../releases/msstore-c-icon-2026-11-04/README.md.
