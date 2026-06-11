#!/usr/bin/env bash
# ClipCat release script: bumps the version, rotates CHANGELOG.md, commits,
# tags and pushes. The CHANGELOG is the gate — a release with an empty
# [Unreleased] section is refused, so user-facing notes are never skipped.
#
# Usage:
#   scripts/release.sh <patch|minor|major|X.Y.Z> [options]
#   scripts/release.sh verify          # changelog/version lint only (CI)
#
# Options:
#   --dry-run        print every step, change nothing
#   --no-push        commit + tag locally, don't push
#   --skip-checks    skip cargo build/clippy/test gates
#   --allow-any-branch  release from a branch other than main
#
# On Windows run it via scripts/release.cmd (uses Git for Windows bash).

set -euo pipefail

err()  { printf 'release: error: %s\n' "$*" >&2; exit 1; }
note() { printf 'release: %s\n' "$*"; }

ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || err "not inside a git repository"
cd "$ROOT"
[ -f Cargo.toml ] || err "Cargo.toml not found at repo root"
[ -f CHANGELOG.md ] || err "CHANGELOG.md not found at repo root"

current_version() {
    sed -n 's/^version = "\([0-9][^"]*\)"/\1/p' Cargo.toml | head -n1
}

# ---- changelog helpers -------------------------------------------------------

# Body of the [Unreleased] section (between its header and the next ## [).
unreleased_body() {
    awk '/^## \[Unreleased\]/{f=1; next} f && /^## \[/{exit} f{print}' CHANGELOG.md
}

verify_changelog() {
    grep -q '^## \[Unreleased\]' CHANGELOG.md \
        || err "CHANGELOG.md has no '## [Unreleased]' section"
    # released sections must look like: ## [X.Y.Z] - YYYY-MM-DD
    local bad
    bad=$(grep '^## \[' CHANGELOG.md | grep -v '^## \[Unreleased\]' \
        | grep -vE '^## \[[0-9]+\.[0-9]+\.[0-9]+\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$' || true)
    [ -z "$bad" ] || err "malformed CHANGELOG section header(s):"$'\n'"$bad"
    # newest released section must match the crate version
    local ver top
    ver=$(current_version)
    [ -n "$ver" ] || err "could not read version from Cargo.toml"
    top=$(grep -m1 -oE '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' CHANGELOG.md | tr -d '#[] ')
    [ "$top" = "$ver" ] \
        || err "Cargo.toml is $ver but newest CHANGELOG section is ${top:-none}"
    note "CHANGELOG.md OK (crate $ver, newest section $top)"
}

# ---- subcommand: verify --------------------------------------------------------

[ $# -ge 1 ] || { sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }

if [ "$1" = "verify" ]; then
    verify_changelog
    exit 0
fi

# ---- parse args ---------------------------------------------------------------

BUMP=$1; shift
DRY_RUN=0 NO_PUSH=0 SKIP_CHECKS=0 ANY_BRANCH=0
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        --no-push) NO_PUSH=1 ;;
        --skip-checks) SKIP_CHECKS=1 ;;
        --allow-any-branch) ANY_BRANCH=1 ;;
        *) err "unknown option: $arg" ;;
    esac
done

CUR=$(current_version)
[ -n "$CUR" ] || err "could not read version from Cargo.toml"
IFS=. read -r MAJ MIN PAT <<EOF
$CUR
EOF
case "$BUMP" in
    major) NEW="$((MAJ + 1)).0.0" ;;
    minor) NEW="$MAJ.$((MIN + 1)).0" ;;
    patch) NEW="$MAJ.$MIN.$((PAT + 1))" ;;
    [0-9]*.[0-9]*.[0-9]*) NEW=$BUMP ;;
    *) err "bump must be patch, minor, major or X.Y.Z (got '$BUMP')" ;;
esac
TAG="v$NEW"
DATE=$(date +%Y-%m-%d)

# ---- preflight ------------------------------------------------------------------

verify_changelog

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ] && [ "$ANY_BRANCH" -eq 0 ]; then
    err "on branch '$BRANCH', not main (use --allow-any-branch to override)"
fi

if [ -n "$(git status --porcelain)" ]; then
    [ "$DRY_RUN" -eq 1 ] || err "working tree is not clean — commit or stash first"
    note "WARNING: working tree dirty (allowed in --dry-run)"
fi

git rev-parse -q --verify "refs/tags/$TAG" >/dev/null && err "tag $TAG already exists"

NOTES=$(unreleased_body | sed -e '/./,$!d')
printf '%s' "$NOTES" | grep -q '^- \|^### ' \
    || err "[Unreleased] in CHANGELOG.md is empty — add user-facing notes first"

note "releasing $CUR -> $NEW (tag $TAG, $DATE) from branch $BRANCH"

# ---- quality gates ---------------------------------------------------------------

if [ "$SKIP_CHECKS" -eq 0 ]; then
    note "running quality gates (cargo build/clippy/test --release)"
    cargo build --release
    cargo clippy --release
    cargo test --release
else
    note "skipping quality gates (--skip-checks)"
fi

if [ "$DRY_RUN" -eq 1 ]; then
    note "dry run — would release $TAG with these notes:"
    printf '%s\n' "$NOTES"
    exit 0
fi

# ---- bump + rotate ---------------------------------------------------------------

# Cargo.toml: first version line is the package version.
sed -i.bak "0,/^version = \"$CUR\"/s//version = \"$NEW\"/" Cargo.toml && rm -f Cargo.toml.bak

# Cargo.lock: keep the clipcat entry in sync without needing the network.
if [ -f Cargo.lock ]; then
    awk -v ver="$NEW" '
        /^name = "clipcat"$/ { found = 1; print; next }
        found && /^version = / { print "version = \"" ver "\""; found = 0; next }
        { print }' Cargo.lock > Cargo.lock.tmp && mv Cargo.lock.tmp Cargo.lock
fi

# CHANGELOG: [Unreleased] content becomes the new version's section.
awk -v ver="$NEW" -v date="$DATE" '
    /^## \[Unreleased\]/ { print; print ""; print "## [" ver "] - " date; next }
    { print }' CHANGELOG.md > CHANGELOG.md.tmp && mv CHANGELOG.md.tmp CHANGELOG.md

git add Cargo.toml CHANGELOG.md
[ -f Cargo.lock ] && git add Cargo.lock
git commit -m "release: $TAG"
git tag -a "$TAG" -m "ClipCat $TAG" -m "$NOTES"
note "committed and tagged $TAG"

# ---- push -------------------------------------------------------------------------

if [ "$NO_PUSH" -eq 1 ]; then
    note "skipping push (--no-push); when ready: git push origin $BRANCH $TAG"
else
    git push origin "$BRANCH" "$TAG"
    note "pushed $BRANCH and $TAG — CI builds the win/mac/linux artifacts"
fi

note "done. release notes for $TAG:"
printf '%s\n' "$NOTES"
