"""Promote main's published release to production; never move it for a build candidate.

Run from a Git checkout with origin, GitHub CLI authentication and Python 3.11+.
No force-reset or local checkout changes. Failed publishing leaves production alone;
if publication succeeded but the ref update failed, rerun the same release workflow.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ('clipcat-windows-x86_64.exe', 'clipcat-macos-arm64.tar.gz')


def command(*args):
    result = subprocess.run(args, cwd=ROOT, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{" ".join(args)} failed: {result.stderr.strip()}')
    return result.stdout.strip()


def git(*args):
    return command('git', *args)


def refresh():
    # checkout can materialize an annotated tag as a local lightweight ref.
    # Refresh local mirrors forcibly; the event-commit guard still rejects a moved tag.
    git('fetch', '--force', 'origin', '+refs/heads/main:refs/remotes/origin/main',
        '+refs/heads/production:refs/remotes/origin/production', '--tags')


def version(commit):
    cargo = git('show', f'{commit}:Cargo.toml')
    package = cargo.split('[package]', 1)[1].split('\n[', 1)[0]
    match = re.search(r'^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"', package, re.M)
    if not match:
        raise RuntimeError('release package version must be X.Y.Z')
    return tuple(map(int, match.groups()))


def check(tag):
    if not re.fullmatch(r'v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', tag):
        raise RuntimeError('release tag must be vX.Y.Z')
    refresh()
    commit = git('rev-parse', f'refs/tags/{tag}^{{commit}}')
    if os.environ.get('GITHUB_REF_TYPE') == 'tag' and git('rev-parse', os.environ.get('GITHUB_SHA', '') + '^{commit}') != commit:
        raise RuntimeError('workflow build commit differs from the release tag')
    previous = git('rev-parse', 'refs/remotes/origin/production')
    git('merge-base', '--is-ancestor', commit, 'refs/remotes/origin/main')
    git('merge-base', '--is-ancestor', previous, commit)
    if tag != 'v' + '.'.join(map(str, version(commit))):
        raise RuntimeError('release tag and Cargo.toml version differ')
    if commit != previous and version(commit) <= version(previous):
        raise RuntimeError('production version must increase')
    return commit, previous


def latest_release():
    return json.loads(command('gh', 'release', 'view', '--json',
                              'tagName,isDraft,isPrerelease,assets'))


def published(tag):
    release = latest_release()
    if release['tagName'] != tag or release['isDraft'] or release['isPrerelease']:
        raise RuntimeError('candidate is not the latest published stable release')
    names = {asset['name'] for asset in release['assets']}
    if not set(ASSETS) <= names:
        raise RuntimeError('published release is missing required Windows/macOS assets')


def ready(tag):
    """Refuse another publication while a previous published release needs recovery."""
    commit, previous = check(tag)
    release = latest_release()
    published(release['tagName'])
    if release['tagName'] != tag:
        live = git('rev-parse', f"refs/tags/{release['tagName']}^{{commit}}")
        if live != previous:
            raise RuntimeError('recover the previous published release before publishing another')
    print(f'release publication ready: {tag} = {commit}')


def promote(tag):
    commit, previous = check(tag)
    published(tag)
    if commit != previous:
        # The explicit lease prevents overwriting any concurrent promotion;
        # check() additionally requires a fast-forward and increasing version.
        git('push', f'--force-with-lease=refs/heads/production:{previous}',
            'origin', f'{commit}:refs/heads/production')
    audit()
    print(f'production = {tag} = {commit}')


def audit():
    release = latest_release()
    published(release['tagName'])
    refresh()
    expected = git('rev-parse', f"refs/tags/{release['tagName']}^{{commit}}")
    actual = git('rev-parse', 'refs/remotes/origin/production')
    if actual != expected:
        raise RuntimeError('production differs from the latest published release; rerun its release job')
    git('merge-base', '--is-ancestor', actual, 'refs/remotes/origin/main')
    if release['tagName'] != 'v' + '.'.join(map(str, version(actual))):
        raise RuntimeError('published tag and production package version differ')
    print(f'production audit OK: {actual}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['check', 'ready', 'promote', 'audit'])
    parser.add_argument('tag', nargs='?')
    args = parser.parse_args()
    if args.action != 'audit' and not args.tag:
        parser.error('check/promote requires a version tag')
    try:
        if args.action == 'audit':audit()
        elif args.action == 'check':print(check(args.tag)[0])
        elif args.action == 'ready':ready(args.tag)
        else:promote(args.tag)
    except (RuntimeError, KeyError, ValueError, IndexError) as error:
        if os.environ.get('GITHUB_ACTIONS'):
            escaped = str(error).replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')
            print(f'::error::production: {escaped}')
        parser.exit(1, f'production: {error}\n')
