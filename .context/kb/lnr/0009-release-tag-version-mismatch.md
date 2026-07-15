# LNR-0009: Auto-update never "took" — release tag built from an un-bumped Cargo.toml

- Date: 2026-07-15 · Area: release process + auto-update (ADR-0009)

## Symptom

On 2.2.0 the update toast appeared ("update available: 2.3.0"), the user picked
**Update to v2.3.0 and restart**, the app downloaded, swapped and relaunched —
and was **still 2.2.0**. The toast came back the next check, and every check
after. From the user's side the update simply never happened, on a loop.

Confusingly, the mechanics all worked: the release existed, the Windows asset
`clipcat-windows-x86_64.exe` was present and downloading (its download counter
climbed), the exe swap and restart ran. Nothing errored.

## Cause

The **released binary reported the wrong version**. Both `v2.2.1` and `v2.3.0`
tags were created by tagging the merge commit `7ea6d5b` directly — **not** via
`scripts/release.sh` — and that commit's `Cargo.toml` still read
`version = "2.2.0"`. So CI built a v2.3.0 release whose binary embeds
`CARGO_PKG_VERSION = "2.2.0"`.

The updater (`src/update.rs`, ADR-0009) is version-driven end to end:

- the check reads the newest tag from the `/releases/latest` redirect (`v2.3.0`)
  and compares it to `env!("CARGO_PKG_VERSION")` of the *running* binary;
- `newer_version("v2.3.0", "2.2.0")` → `Some("2.3.0")` → prompt.

The swap genuinely replaces the exe — but the new exe *also* self-identifies as
`2.2.0`, because it was built from the un-bumped tree. So after restart the very
same comparison fires again, forever. The update "not working" was never the
download/swap/restart path; it was that **the thing installed was, by its own
version string, indistinguishable from what was already running.**

`scripts/release.sh` would have prevented this — it bumps `Cargo.toml` (and
`Cargo.lock`, and rotates the CHANGELOG) in the *same commit* it tags, so a
release tag can never point at an un-bumped tree. These two tags bypassed it.

## Fix

Enforce the invariant at the last checkpoint before assets are published — a CI
step on every `v*` build that fails unless the tag equals the crate version:

```yaml
# .github/workflows/ci.yml — build job, right after checkout
- name: Verify tag matches Cargo.toml version
  if: startsWith(github.ref, 'refs/tags/v')
  shell: bash
  run: |
    tag_ver="${GITHUB_REF_NAME#v}"
    crate_ver=$(sed -n 's/^version = "\([0-9][^"]*\)".*/\1/p' Cargo.toml | head -n1)
    [ "$tag_ver" = "$crate_ver" ] || { echo "::error::tag $GITHUB_REF_NAME != Cargo.toml $crate_ver"; exit 1; }
```

`build` fails → the `release` job (`needs: build`) is skipped → no mis-versioned
asset is ever published, no matter how the tag was created (release script,
`git tag`, or the GitHub UI).

**Recovering the already-published releases is a separate, manual step** (no code
change can re-version a binary already on a user's disk): cut a *new* release
above `2.3.0` with `scripts/release.sh 2.3.1` (an explicit `X.Y.Z` is required —
`patch`→`2.2.1` and `minor`→`2.3.0` both refuse, those tags already exist). Its
binary then reports `2.3.1`, the redirect resolves to `v2.3.1`, and users on the
looping build finally settle. The stale `v2.2.1`/`v2.3.0` assets can be left or
re-uploaded from a correctly-versioned build.

## Takeaway

- **A release tag and the binary's `CARGO_PKG_VERSION` are one fact in two
  places; drift makes the updater loop.** The updater trusts the tag to name
  what the binary *is*. If they disagree, "install" is a no-op the user can't see
  and the prompt never clears.
- **Always cut releases with `scripts/release.sh`.** It's the only path that
  bumps the version in the tagged commit. A hand-made `git tag`/GitHub-UI release
  on a feature/merge commit skips the bump silently.
- **Guard invariants at the last gate that can still stop the harm.** The bump
  belongs to the release script, but the *enforcement* belongs in CI on the tag
  build — the one place every release passes through regardless of how its tag
  was born.
