# macOS clipboard and updater validation — 2026-10-31

Host: Apple Silicon Mac, macOS 27. Native release builds were exercised with
synthetic data. The user's ClipCat profile was not used: a temporary source
copy changed only the profile directory for the native clipboard tests.

## Clipboard

`scripts/validate_macos_paste.swift` drove a real Cocoa NSTextView with an
Edit/Paste menu and a real ClipCat window. Accessibility was available.

- Return selects the previous clip, copies it, dismisses the flyout and pastes
  into the original app.
- Reopening that same flyout and selecting with keypad Enter works.
- Cmd+Return removes HTML/RTF from the rich synthetic clip and pastes plain text.
- Native main-thread activation was observed before paste; no user input hook
  or AX element text reading was used.

Results: [paste-result.json](paste-result.json),
[actual synthetic target window](paste-target.png). The screenshot is an actual
native window capture after paste, not a renderer preview. Cocoa advertises both
`public.utf8-plain-text` and its legacy `NSStringPboardType` alias for plain text.

## Updater

Both standalone and bundled installations were exercised end to end using a
locally signed, arm64 executable that writes a restart marker and exits. The
archive was supplied by a local `curl` substitute that required the expected
GitHub download URL; this validates download invocation, extraction, signature/
version/CPU validation, replacement, old-process exit, and detached relaunch.
No GitHub release was published or installed into the user's Applications.
Installed paths included spaces and apostrophes. `/private/tmp` is macOS's
canonical form of `/tmp` for the bundled launch.

Results: [update-restart-result.json](update-restart-result.json).
Unit tests separately reject a mismatched version and damaged signature without
replacing the existing installation. A universal release ZIP was generated from
native arm64 and cross-built x86_64 binaries and passed `codesign --verify`.
Intel execution, Windows execution and a real public-release upgrade were not
performed. The release remains ad-hoc signed, without Developer ID/notarization.

## Automated checks

- `cargo test --locked --release`: 218 passed (185 library + 30 integration + 3 release-script).
- Native release build, Intel cross-build, universal packaging and Clippy run.
  Clippy has pre-existing warnings in clipboard/render/portable/gen_icon and the
  pre-existing `block 0.1.6` dependency future-compatibility warning.
- `bash scripts/release.sh verify` and `git diff --check` pass.
- The unchanged Python release-policy suite has one environment-related error:
  `test_concurrent_ref_change_is_not_overwritten` reaches its existing `gh`
  audit path, but GitHub CLI is not installed on this Mac. The other 12 pass.
  This test and `scripts/production.py` were not changed by this fix.

All probe apps launched for these checks were terminated or exited themselves.
