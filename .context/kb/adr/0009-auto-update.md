# 0009 — Auto-update via GitHub releases (system curl, exe swap on Windows)

Status: Accepted · Date: 2026-06-12

## Context

Releases are git tags cut by `scripts/release.sh`, but users had no way to
learn a new version exists short of revisiting the repo. Two golden rules
stood in the way: **no network code** (privacy — golden rule 1) and **no new
heavy dependencies** (an HTTP+TLS stack is exactly that). The owner asked
for auto-update on the premise that GitHub releases are the distribution
channel.

## Decision

- **GitHub Releases are the update channel.** CI publishes every `v*` tag
  as a GitHub Release with **stable asset names**
  (`clipcat-windows-x86_64.exe`, `clipcat-macos-arm64.tar.gz`);
  `src/update.rs` derives every URL from `Cargo.toml`'s `repository` field.
- **The version check is a redirect probe, not an API call**:
  `GET <repo>/releases/latest` redirects to `/releases/tag/vX.Y.Z`; the tag
  is parsed from the final URL. No JSON, no auth, no rate-limit worries at
  one check per day. Assumes a public repo (a private fork simply never
  sees updates).
- **Networking is delegated to the system `curl`** (ships with Windows 10
  1803+, macOS, and effectively every Linux) spawned via `std::process`:
  zero new crates, no TLS stack in the binary. Missing curl degrades to
  "no update check", silently.
- A background thread checks ~10 s after launch, then every 24 h, gated by
  the persisted `auto_update` setting (default **on**; tray-menu toggle on
  Windows, `state.json` on portable). A found version surfaces as a toast
  and a tray entry — **nothing downloads without a user action**.
- **Windows (native)**: "Update to vX.Y.Z and restart" downloads the exe
  asset to `%TEMP%` on a worker thread, sanity-checks the PE magic, renames
  the running exe to `<exe>.old` (Windows allows that), copies the new one
  into place, then relaunches via a detached `cmd /c ping … & start …`
  helper (~1 s delay frees the singleton mutex) and exits. The `.old` file
  is removed on next start.
- **Linux portable**: `U` after the update toast opens the releases
  page in the browser. Self-replacing a downloaded naked binary there
  (quarantine, exec bits, arch variants) is not worth the complexity for
  the non-premium platforms.

The privacy rule is **amended, not dropped**: `src/update.rs` is the single
sanctioned network exception; it contacts github.com only and transmits
nothing beyond the HTTPS request itself. Clipboard/input data still never
leaves the machine, and the About dialog now discloses the check instead of
claiming "no network, ever".

## Consequences

- `ci.yml`'s asset names and `update::WINDOWS_ASSET` are a contract; so is
  `Cargo.toml`'s `repository` URL pointing at the public repo.
- CI gained a `release` publish job (artifact download + `gh release
  create`); release notes come from the annotated tag that
  `scripts/release.sh` already writes.
- Version parsing/comparison and redirect-tag extraction are unit-tested;
  the real network/file-swap path can only be validated manually on a
  Windows machine (install an older build, publish a release, click the
  tray entry).
- An aborted swap rolls back (`.old` renamed back); a failed download
  leaves the running install untouched and toasts "UPDATE FAILED".

## macOS revision — 2026-10-31

The macOS update action now installs and restarts instead of opening a browser.
CI publishes `clipcat-macos-universal.zip` containing an ad-hoc signed universal
`ClipCat.app`; the older arm64 tarball remains available. The updater validates
the bundle signature, requested version and host CPU before replacing anything.
A running `.app` is replaced as a whole, preserving its signed structure;
standalone installs take the executable from that same archive. A sibling stage
keeps renames on one volume, restores the old install if replacement fails, and
retains a backup if rollback fails. No privileged helper is installed: read-only
locations fail with the existing retry notification. A detached helper waits for
process exit before reopening the original installation path.

Downloads remain user-initiated and local history/settings are preserved. This
is still ad-hoc signing, without Developer ID/notarization; macOS may require
Accessibility approval again after an update. Existing macOS releases have no
self-installer, so users must install the first release carrying this fix once.
