# Microsoft Store Release Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task by task.

**Goal:** Prepare and submit ClipCat through the in-app Partner Center browser.

**Architecture:** Package a Windows x64 Store build from an exact production
release commit as MSIX. A dependency-free `store` Cargo feature disables the
GitHub updater and the legacy registry autostart controls in that build. Default
GitHub builds retain their existing behavior. Use the Microsoft MSIX SDK or
Windows SDK to validate the package; never claim Windows installation tests that
were not run.

**Tech Stack:** Rust, Python standard library, Windows SDK/MSIX SDK, GitHub CI.

**Spec:** AGENTS.md; docs/releasing.md; Partner Center product 9NZBR8X5T677.

## Constraints and review focus

- Production must equal the latest public release tag; promote through release CI.
- Store identity: PIXO.ClipCat; publisher CN=62E0CEB7-8FB8-4E4C-BF40-5231FBBC83E0;
  publisher display name PIXO. Languages en-US and ko-KR. Desktop x64 only.
- No additional app network features, runtime dependencies, or bundled fonts.
- Old saved auto_update=true must not reactivate Store self-update.
- Store builds must not offer registry autostart or GitHub update installation.
- Keep the native Win32 and portable menus consistent.
- Existing clipboard data and capture/paste behavior must remain compatible.
- Distinguish preview screenshots from actual Windows package execution.

## Steps

- [x] Add a failing Store regression test proving saved settings cannot enable
  the updater and menus do not offer unsupported distribution controls.
- [x] Add the `store` Cargo feature and gate updater entry points and menus.
- [x] Add exact-identity MSIX packaging and a Windows CI job, building only the
  published production commit, with package validation and launch smoke test.
- [x] Run release build, Clippy, existing tests, Store tests, release lint and
  packaging checks. Review the changes and commit/push over SSH.
- [x] Use scripts/release.sh to publish the required compatibility release;
  wait for successful CI and audit production/tag equality.
- [x] Download the validated Store package, upload it through the in-app browser,
  and fill English/Korean listings with truthful screenshots and privacy info.
- [x] Complete IARC only after permission to transmit publisher/email is received.
- [x] Submit for certification when all required items are complete; record
  the exact package SHA, source tag/SHA and observed Partner Center state.

## Initial observations, 2026-11-03

Product created and name reserved. The first submission was created in draft.
Free worldwide pricing and Productivity/Personalization properties saved.
Initial candidate v2.5.2, commit 32ab0b05834691fdd51f2a92116f2b2ed2f42e42,
has a successful release workflow 41465852278, but is an unpackaged build whose
self-updater replaces its executable. A Store compatibility build is required.
IARC preview was blocked by automatic approval review pending explicit approval
for publisher display name/email transmission; do not retry until approved.

## Verification ledger

- Store regression tests failed before the guards were implemented, then passed.
- macOS default and Store suites each passed 185 unit + 30 integration + 3 release
  script tests. CoreGraphics event construction required unsandboxed execution.
- Default release build and both Clippy modes passed. Existing gen_icon warnings
  and upstream block future-incompatibility warning remain; no checks weakened.
- Production Python tests passed (13), with GitHub CLI present on PATH.
- English/Korean panel previews and the generated Store icon visually inspected.
- Independent review found a 30-second autosave polling mismatch, ambiguous window
  lookup, and fork workflow_run privilege exposure. Fixed polling to 60 seconds,
  matched the main window caption, required a same-repository push, and checked
  source/production/public tag equality before executing repository scripts.
- User approved IARC publisher/email sharing and separately approved the Terms
  of Use/adulthood confirmation. Ratings saved; age ratings is Complete.
- English and Korean listing text drafts saved; screenshots still required.
- Ruling: use the Windows SDK on CI instead of building the cross-platform SDK
  locally; it validates the native manifest using Microsoft's shipping tool.
- Ruling: smoke-test the staged exe with synthetic clipboard data and clearly
  report that MSIX deployment and WACK are not tested by this workflow.
- Released v2.5.3 through scripts/release.sh. Windows/macOS release CI succeeded;
  production and tag equal b3fcf9280a6bde01f5cea55d66470ae7da69bd0f.
- Store package CI 41657676435 passed MakeAppx and real native clipboard smoke.
  Screenshot CI 41658759031 unpacked the exact published MSIX and repeated smoke
  with the CI terminal minimized; this clean screenshot was used in both listings.
- Partner Center validated MSIX 2.5.3.0. All six required sections Complete.
  Submitted 2026-11-04 00:06 KST; observed In certification / Pre-processing.
  Receipt and limits: ../../releases/msstore-2026-11-04/submission.json.
