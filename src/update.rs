//! Optional self-update against GitHub releases (ADR-0009).
//!
//! Releases are git tags (`scripts/release.sh`); CI publishes each `vX.Y.Z`
//! tag as a GitHub Release with stable asset names. A background thread
//! probes `<repository>/releases/latest` once a day — GitHub redirects that
//! URL to `/releases/tag/vX.Y.Z`, so the newest version is read from the
//! final URL without any API/JSON. Networking is delegated to the system
//! `curl` (present on Windows 10+, macOS and practically every Linux):
//! zero new crates, no TLS stack in the binary, and if curl is missing the
//! check silently does nothing.
//!
//! This module is the single sanctioned exception to the "no network code"
//! rule: it contacts github.com only, transmits nothing beyond the request
//! itself, and is gated by the persisted `auto_update` setting. Applying an
//! update is always user-initiated: the native Windows backend downloads
//! the released exe and swaps itself, the portable backend opens the
//! releases page in the browser.

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// `https://github.com/<owner>/<repo>` from Cargo.toml.
const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Release asset the Windows self-updater downloads. Must stay in sync with
/// the "Prepare release asset" step in `.github/workflows/ci.yml`.
pub const WINDOWS_ASSET: &str = "clipcat-windows-x86_64.exe";

/// Daily cadence; the first check runs shortly after launch.
const CHECK_PERIOD: Duration = Duration::from_secs(24 * 60 * 60);

/// Mirrors `Persist::auto_update`; while false no network request is made.
static ENABLED: AtomicBool = AtomicBool::new(true);
/// Newer version found by the checker ("2.1.0"), until a backend takes it.
static FOUND: Mutex<Option<String>> = Mutex::new(None);
/// Install state machine (see [`Install`]); written by the install worker.
static INSTALL: AtomicU8 = AtomicU8::new(INSTALL_IDLE);

const INSTALL_IDLE: u8 = 0;
const INSTALL_WORKING: u8 = 1;
const INSTALL_READY: u8 = 2;
const INSTALL_FAILED: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Install {
    Idle,
    Working,
    /// The new exe is in place; restart the app to finish.
    Ready,
    Failed,
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// The version a background check found, handed out once (backends call
/// this every tick and forward it to [`crate::pet::Pet::notify_update`]).
pub fn take_found() -> Option<String> {
    FOUND.lock().ok()?.take()
}

/// Terminal install states are consumed on read so the backend reacts to
/// each exactly once (Ready → restart, Failed → toast + retry possible).
pub fn poll_install() -> Install {
    match INSTALL.load(Ordering::Acquire) {
        INSTALL_WORKING => Install::Working,
        INSTALL_READY => {
            INSTALL.store(INSTALL_IDLE, Ordering::Release);
            Install::Ready
        }
        INSTALL_FAILED => {
            INSTALL.store(INSTALL_IDLE, Ordering::Release);
            Install::Failed
        }
        _ => Install::Idle,
    }
}

/// Spawns the daily release check. Call once at startup, after
/// [`set_enabled`] has been fed the persisted setting.
pub fn spawn_checker() {
    std::thread::spawn(|| {
        // let startup (window, hooks, first paint) settle first
        std::thread::sleep(Duration::from_secs(10));
        loop {
            if ENABLED.load(Ordering::Relaxed) {
                if let Some(tag) = latest_tag() {
                    if let Some(v) = newer_version(&tag, env!("CARGO_PKG_VERSION")) {
                        if let Ok(mut found) = FOUND.lock() {
                            *found = Some(v);
                        }
                    }
                }
                std::thread::sleep(CHECK_PERIOD);
            } else {
                // re-read the toggle hourly so enabling it acts the same day
                std::thread::sleep(Duration::from_secs(60 * 60));
            }
        }
    });
}

/// Opens the releases page in the default browser — how the portable
/// backend "applies" an update (the user downloads the right build).
pub fn open_releases_page() {
    open_url(&format!("{REPO_URL}/releases/latest"));
}

/// The project's GitHub page, linked from the context menu. Placeholder for
/// now (the real org/repo isn't published yet).
pub const GITHUB_URL: &str = "https://github.com/clipcat";

/// Opens the GitHub page (the context-menu "GitHub" item).
pub fn open_github() {
    open_url(GITHUB_URL);
}

/// Opens `url` in the user's default browser. This launches the OS browser
/// (open / xdg-open / `start`); the app itself transmits nothing, so it is not
/// a network use in the ADR-0009 sense.
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = command("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = command("xdg-open").arg(url).spawn();
    #[cfg(windows)]
    let _ = command("cmd").args(["/c", "start", "", url]).spawn();
}

// ---- version check ----------------------------------------------------------

/// Tag of the newest GitHub release, read from the `/releases/latest`
/// redirect. None on any failure (offline, no releases yet, no curl).
fn latest_tag() -> Option<String> {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let url = format!("{REPO_URL}/releases/latest");
    let final_url = curl(&[
        "-fsSL", "--max-time", "20", "-o", null, "-w", "%{url_effective}", &url,
    ])?;
    tag_from_redirect(&final_url)
}

/// ".../releases/tag/v2.1.0" -> "v2.1.0"; anything that is not a version
/// tag (e.g. no redirect happened) is rejected.
fn tag_from_redirect(url: &str) -> Option<String> {
    let tag = url.trim().rsplit('/').next()?;
    (tag.starts_with('v') && parse_version(tag).is_some()).then(|| tag.to_string())
}

/// "v2.1.0" / "2.1.0" -> (2, 1, 0); anything else -> None.
fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let mut parts = s.split('.');
    let v = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(v)
}

/// The bare version ("2.1.0") when `tag` is strictly newer than `current`.
fn newer_version(tag: &str, current: &str) -> Option<String> {
    (parse_version(tag)? > parse_version(current)?)
        .then(|| tag.strip_prefix('v').unwrap_or(tag).to_string())
}

// ---- process helpers ----------------------------------------------------------

/// A `Command` that never flashes a console window on Windows.
fn command(program: &str) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Runs the system curl; stdout on success, None on any failure.
fn curl(args: &[&str]) -> Option<String> {
    let out = command("curl").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

// ---- Windows self-install -----------------------------------------------------

/// Sibling path the running exe is renamed to during the swap.
#[cfg(windows)]
fn old_exe_path(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let name = exe.file_name()?.to_str()?;
    Some(exe.with_file_name(format!("{name}.old")))
}

/// Removes the `<exe>.old` left behind by a previous update (best-effort;
/// call once at startup).
#[cfg(windows)]
pub fn cleanup_old_exe() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(old) = old_exe_path(&exe) {
            let _ = std::fs::remove_file(old);
        }
    }
}

/// Downloads release `version` and swaps the running exe, on a worker
/// thread; progress lands in [`poll_install`]. User-initiated only.
#[cfg(windows)]
pub fn begin_install(version: &str) {
    if INSTALL
        .compare_exchange(
            INSTALL_IDLE,
            INSTALL_WORKING,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        return; // an install is already in flight
    }
    let version = version.to_string();
    std::thread::spawn(move || {
        let ok = install(&version).is_some();
        INSTALL.store(
            if ok { INSTALL_READY } else { INSTALL_FAILED },
            Ordering::Release,
        );
    });
}

#[cfg(windows)]
fn install(version: &str) -> Option<()> {
    let url = format!("{REPO_URL}/releases/download/v{version}/{WINDOWS_ASSET}");
    let tmp = std::env::temp_dir().join(format!("clipcat-update-{version}.exe"));
    let tmp_str = tmp.to_str()?.to_string();
    curl(&["-fsSL", "--max-time", "600", "-o", &tmp_str, &url])?;

    // sanity: a real PE executable starts with "MZ"
    let mut magic = [0u8; 2];
    use std::io::Read;
    std::fs::File::open(&tmp).ok()?.read_exact(&mut magic).ok()?;
    if &magic != b"MZ" {
        let _ = std::fs::remove_file(&tmp);
        return None;
    }

    // Windows lets a running exe be renamed (its mapping stays valid), so
    // the new binary can take the original path; .old is removed next start.
    let exe = std::env::current_exe().ok()?;
    let old = old_exe_path(&exe)?;
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).ok()?;
    if std::fs::copy(&tmp, &exe).is_err() {
        let _ = std::fs::rename(&old, &exe); // roll back
        let _ = std::fs::remove_file(&tmp);
        return None;
    }
    let _ = std::fs::remove_file(&tmp);
    Some(())
}

/// Relaunches the (already swapped) exe after this process exits: a
/// detached helper waits ~1 s — by then the singleton mutex is free — and
/// starts it. Call right before tearing the app down.
#[cfg(windows)]
pub fn restart() {
    use std::os::windows::process::CommandExt;
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = command("cmd");
    cmd.arg("/c");
    // raw_arg: std's MSVCRT-style quoting would mangle cmd.exe syntax
    cmd.raw_arg(format!(
        "ping -n 2 127.0.0.1 >nul & start \"\" \"{}\"",
        exe.display()
    ));
    let _ = cmd.spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_versions() {
        assert_eq!(parse_version("v2.1.0"), Some((2, 1, 0)));
        assert_eq!(parse_version("10.20.30"), Some((10, 20, 30)));
        assert_eq!(parse_version("v2.1"), None);
        assert_eq!(parse_version("v2.1.0.4"), None);
        assert_eq!(parse_version("v2.1.x"), None);
        assert_eq!(parse_version("latest"), None);
    }

    #[test]
    fn newer_version_is_strict_semver() {
        assert_eq!(newer_version("v2.1.0", "2.0.0").as_deref(), Some("2.1.0"));
        assert_eq!(newer_version("v3.0.0", "2.9.9").as_deref(), Some("3.0.0"));
        assert_eq!(newer_version("v2.0.1", "2.0.0").as_deref(), Some("2.0.1"));
        assert_eq!(newer_version("v2.0.0", "2.0.0"), None, "same is not newer");
        assert_eq!(newer_version("v1.9.9", "2.0.0"), None, "never downgrade");
        assert_eq!(newer_version("garbage", "2.0.0"), None);
    }

    #[test]
    fn tag_comes_from_the_redirect_url_only() {
        assert_eq!(
            tag_from_redirect("https://github.com/o/r/releases/tag/v2.1.0").as_deref(),
            Some("v2.1.0")
        );
        // no releases yet / no redirect followed: not a version tag
        assert_eq!(tag_from_redirect("https://github.com/o/r/releases"), None);
        assert_eq!(
            tag_from_redirect("https://github.com/o/r/releases/latest"),
            None
        );
        assert_eq!(tag_from_redirect(""), None);
    }

    #[test]
    fn current_crate_version_is_parseable() {
        // guards against a Cargo.toml version the checker could not compare
        assert!(parse_version(env!("CARGO_PKG_VERSION")).is_some());
    }
}
