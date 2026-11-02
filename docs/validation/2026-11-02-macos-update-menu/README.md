# macOS update-menu follow-up — 2026-11-02

## Diagnosis

The running Desktop `ClipCat.app` was a **2.4.0 preview from commit 5f0f8f5**.
Its executable did not contain the universal self-installer added in `905a253`.
Pushing that fix to main had not updated the installed preview.

At diagnosis, the latest public release was v2.5.0 (published 2026-10-31).
It contained only `clipcat-macos-arm64.tar.gz` and the Windows exe, without
`clipcat-macos-universal.zip`. No release was published during this follow-up.
Old previews need a manual bootstrap to the build carrying the installer.

## Actual context-menu verification

A temporary source copy used a separate synthetic profile and advertised a
fixture version 2.5.1. The production menu/installer code was unchanged; temporary
diagnostics logged the selected action. A local curl substitute supplied the
signed fixture archive and required the expected GitHub download URL.

The probe right-clicked the actual cat window and **mouse-clicked the visible
"Update to v2.5.1 and restart" item**. Logs confirmed `InstallUpdate`, the worker
completed, the original app exited normally, and the replacement executable
created its restart marker. See [menu-result.json](menu-result.json) and
[actual test menu capture](context-menu.png). Version 2.5.1 in that screenshot is
synthetic; it is not a published release.

Keyboard/AX automation attempts did not select this contextual item. The final
passing check used its actual screen rectangle and a mouse click. All temporary
probe apps exited or were terminated. This does not claim a public-release
upgrade or a new Windows execution check.

## Local bootstrap and verification

- Built native arm64 and cross-target x86_64 release binaries from `905a253`.
- Packaged the universal preview with the existing `dev.clipcat.preview` identity;
  `codesign --verify --deep --strict` passed.
- Backed up the installed app and profile under
  `~/Library/Application Support/ClipCat/app-backups/2026-11-02/`.
- Used ClipCat's own Escape/Q handling to save and exit the old process before
  replacing `~/Desktop/ClipCat.app` and relaunching it.
- Confirmed the new executable is running; installed metadata reports 2.5.0,
  `Cat companions test build 905a253`.
- `clips.json` and `requests.json` match the backup byte-for-byte (SHA-256).
  No clipboard contents were printed or included in these records.

This is still an ad-hoc signed development preview, not a newly published release.
The application was left running for its normal desktop/clipboard use. No runtime
source change was necessary for this follow-up.
