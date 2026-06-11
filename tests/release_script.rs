//! End-to-end test of `scripts/release.sh` against a scratch git repository:
//! version bump, Cargo.lock sync, CHANGELOG rotation, commit, annotated tag,
//! the empty-[Unreleased] gate and the `verify` lint. Unix-only (the script
//! is bash; on Windows it runs through scripts/release.cmd + Git bash, which
//! CI exercises only as far as the same release.sh).
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(dir: &Path, program: &str, args: &[&str]) -> (bool, String) {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {program}: {e}"));
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), text)
}

fn git(dir: &Path, args: &[&str]) -> (bool, String) {
    run(dir, "git", args)
}

/// A minimal clipcat-shaped repo with the real release.sh copied in.
fn scratch_repo(name: &str) -> Scratch {
    let dir = std::env::temp_dir().join(format!(
        "clipcat-release-test-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("scripts")).unwrap();

    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"clipcat\"\nversion = \"2.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"clipcat\"\nversion = \"2.0.0\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n### Added\n- A user-facing feature.\n\n\
         ## [2.0.0] - 2026-06-12\n\n### Added\n- Everything.\n",
    )
    .unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/release.sh");
    std::fs::copy(script, dir.join("scripts/release.sh")).unwrap();

    assert!(git(&dir, &["init", "-q", "-b", "main"]).0);
    assert!(git(&dir, &["config", "user.email", "test@example.com"]).0);
    assert!(git(&dir, &["config", "user.name", "Release Test"]).0);
    // a host-level signing setup must not leak into the scratch repo
    assert!(git(&dir, &["config", "commit.gpgsign", "false"]).0);
    assert!(git(&dir, &["config", "tag.gpgsign", "false"]).0);
    assert!(git(&dir, &["add", "-A"]).0);
    assert!(git(&dir, &["commit", "-q", "-m", "init"]).0);
    Scratch(dir)
}

fn release(dir: &Path, args: &[&str]) -> (bool, String) {
    let mut all = vec!["scripts/release.sh"];
    all.extend_from_slice(args);
    run(dir, "bash", &all)
}

#[test]
fn release_bumps_rotates_commits_and_tags() {
    let repo = scratch_repo("happy");
    let dir = &repo.0;

    // the changelog lint passes on the starting state
    let (ok, out) = release(dir, &["verify"]);
    assert!(ok, "verify failed:\n{out}");

    let (ok, out) = release(dir, &["minor", "--skip-checks", "--no-push"]);
    assert!(ok, "release failed:\n{out}");

    let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"2.1.0\""), "Cargo.toml:\n{cargo}");
    let lock = std::fs::read_to_string(dir.join("Cargo.lock")).unwrap();
    assert!(lock.contains("version = \"2.1.0\""), "Cargo.lock:\n{lock}");

    let log = std::fs::read_to_string(dir.join("CHANGELOG.md")).unwrap();
    assert!(log.contains("## [Unreleased]"), "keeps an empty Unreleased");
    assert!(log.contains("## [2.1.0] - "), "rotated section:\n{log}");
    let unreleased_to_2_1 = log.find("## [Unreleased]").unwrap() < log.find("## [2.1.0]").unwrap();
    assert!(unreleased_to_2_1, "Unreleased stays on top");

    // clean tree, release commit, annotated tag carrying the notes
    let (_, status) = git(dir, &["status", "--porcelain"]);
    assert!(status.trim().is_empty(), "dirty after release: {status}");
    let (_, subject) = git(dir, &["log", "-1", "--format=%s"]);
    assert_eq!(subject.trim(), "release: v2.1.0");
    let (ok, tag) = git(dir, &["tag", "-l", "-n100", "v2.1.0"]);
    assert!(ok && tag.contains("A user-facing feature."), "tag notes:\n{tag}");

    // verify still passes against the released state
    let (ok, out) = release(dir, &["verify"]);
    assert!(ok, "post-release verify failed:\n{out}");

    // a second release without new notes is refused: that's the gate that
    // keeps CHANGELOG maintenance honest
    let (ok, out) = release(dir, &["patch", "--skip-checks", "--no-push"]);
    assert!(!ok, "must refuse an empty [Unreleased]");
    assert!(out.contains("[Unreleased]"), "explains the gate:\n{out}");
}

#[test]
fn release_guards_branch_dirty_tree_and_bad_input() {
    let repo = scratch_repo("guards");
    let dir = &repo.0;

    // wrong branch
    assert!(git(dir, &["checkout", "-q", "-b", "feature"]).0);
    let (ok, out) = release(dir, &["patch", "--skip-checks", "--no-push"]);
    assert!(!ok && out.contains("main"), "branch guard:\n{out}");
    // ...unless explicitly allowed (also proves the override works)
    let (ok, out) = release(
        dir,
        &["patch", "--skip-checks", "--no-push", "--allow-any-branch"],
    );
    assert!(ok, "allow-any-branch failed:\n{out}");
    assert!(git(dir, &["checkout", "-q", "main"]).0);

    // dirty tree
    std::fs::write(dir.join("dirty.txt"), "x").unwrap();
    let (ok, out) = release(dir, &["patch", "--skip-checks", "--no-push"]);
    assert!(!ok && out.contains("clean"), "dirty-tree guard:\n{out}");
    std::fs::remove_file(dir.join("dirty.txt")).unwrap();

    // nonsense bump specifier
    let (ok, out) = release(dir, &["banana", "--skip-checks", "--no-push"]);
    assert!(!ok && out.contains("bump"), "bump guard:\n{out}");

    // dry run changes nothing
    let before = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    let (ok, out) = release(dir, &["major", "--dry-run", "--skip-checks"]);
    assert!(ok, "dry-run failed:\n{out}");
    assert!(out.contains("3.0.0"), "announces the version:\n{out}");
    let after = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert_eq!(before, after, "dry run must not write");
    let (_, tags) = git(dir, &["tag"]);
    assert!(!tags.contains("v3.0.0"));
}

#[test]
fn verify_catches_version_drift() {
    let repo = scratch_repo("drift");
    let dir = &repo.0;
    // Cargo.toml says 9.9.9 but the newest changelog section is 2.0.0
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"clipcat\"\nversion = \"9.9.9\"\n",
    )
    .unwrap();
    let (ok, out) = release(dir, &["verify"]);
    assert!(!ok && out.contains("9.9.9"), "drift not caught:\n{out}");
}
