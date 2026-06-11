---
name: release
description: Cut a ClipCat release - validate the CHANGELOG, run the quality gates, bump the version, tag and push via scripts/release.sh. Use when asked to release, ship, publish, tag a version, or bump the version.
---

# ClipCat release

Releases are driven by `scripts/release.sh` (on Windows: `scripts\release.cmd`,
which forwards to the same script via Git for Windows bash). The script bumps
`Cargo.toml`/`Cargo.lock`, rotates the `[Unreleased]` section of
`CHANGELOG.md` into the new version, commits, tags `vX.Y.Z` and pushes.

## Steps

1. **Pick the bump.** Read `## [Unreleased]` in `CHANGELOG.md`:
   - only `Fixed` bullets → `patch`
   - any `Added`/`Changed` bullet → `minor`
   - breaking behavior/data-format changes → `major`
   If the user named an explicit version, pass `X.Y.Z` instead.

2. **Check the changelog gate.** `[Unreleased]` must contain at least one
   bullet, written for **users** (features, behavior changes, fixes). Move
   any dev-only notes (CI, refactors, docs) out — they don't belong in the
   changelog. If `[Unreleased]` is empty, stop and ask whether there is
   really anything to release.

3. **Dry-run first** from a clean tree on `main`:

   ```bash
   scripts/release.sh <bump> --dry-run
   ```

   This also runs the quality gates (`cargo build/clippy/test --release`).
   Fix anything that fails; never release with `--skip-checks` unless the
   user explicitly accepts that.

4. **Release.**

   ```bash
   scripts/release.sh <bump>
   ```

   Use `--no-push` when the user wants to inspect the commit/tag before it
   goes out (then `git push origin main vX.Y.Z`).

5. **Afterwards.** CI builds the Windows/macOS/Linux artifacts for the
   pushed tag. If asked to publish a GitHub release, use the tag's release
   notes (the script prints them; they are also in the annotated tag).

## Sanity rules

- Never edit version numbers or rotate the changelog by hand — the script
  is the single path, and `scripts/release.sh verify` is the CI lint.
- Releases happen from `main`; `--allow-any-branch` is for emergencies only.
- If the tag already exists or the tree is dirty the script aborts; resolve
  the cause rather than forcing.
