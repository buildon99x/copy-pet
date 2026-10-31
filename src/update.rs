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
//! the released exe and swaps itself, macOS installs a universal app/binary and restarts; Linux opens the
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
#[cfg(target_os = "macos")]
pub const MACOS_ASSET: &str = "clipcat-macos-universal.zip";

/// Daily cadence; the first check runs shortly after launch.
const CHECK_PERIOD: Duration = Duration::from_secs(24 * 60 * 60);
/// Shorter back-off after a *failed* check (e.g. offline at boot) so a
/// transient failure isn't stuck waiting a full day to retry.
const RETRY_PERIOD: Duration = Duration::from_secs(15 * 60);

/// Mirrors `Persist::auto_update`; while false no network request is made.
static ENABLED: AtomicBool = AtomicBool::new(true);
/// Newer version found by the checker ("2.1.0"), until a backend takes it.
static FOUND: Mutex<Option<String>> = Mutex::new(None);
/// Install state machine (see [`Install`]); written by the install worker.
static INSTALL: AtomicU8 = AtomicU8::new(INSTALL_IDLE);
#[cfg(target_os = "macos")]
static RESTART_EXE: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

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
                let ok = if let Some(tag) = latest_tag() {
                    if let Some(v) = newer_version(&tag, env!("CARGO_PKG_VERSION")) {
                        if let Ok(mut found) = FOUND.lock() {
                            *found = Some(v);
                        }
                    }
                    true
                } else {
                    false
                };
                // Full daily cadence on success; short back-off on failure.
                std::thread::sleep(if ok { CHECK_PERIOD } else { RETRY_PERIOD });
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

/// Opens the project's GitHub page (the context-menu "GitHub" item). Uses the
/// repository URL from Cargo.toml — the same source the updater builds its
/// release URLs from (ADR-0009) — so there's no second address to hardcode or
/// keep in sync.
pub fn open_github() {
    open_url(REPO_URL);
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
#[cfg(any(windows, target_os = "macos"))]
pub fn begin_install(version: &str) {
    if parse_version(version).is_none() { return; }
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

// ---- macOS self-install ------------------------------------------------------

/// Only treat the executable as bundled when it is in Contents/MacOS.
#[cfg(target_os = "macos")]
fn macos_bundle(exe: &std::path::Path) -> Option<&std::path::Path> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents"
        && app.extension()? == "app").then_some(app)
}

/// Stage beside the installed path, so the final renames stay on one volume.
/// The old executable/bundle remains mapped while running. Restore it if the
/// candidate cannot be moved into place; never edit a signed bundle in place.
#[cfg(target_os = "macos")]
fn replace_macos_path(source: &std::path::Path, installed: &std::path::Path) -> std::io::Result<()> {
    std::fs::symlink_metadata(source)?;
    let stage = installed.with_file_name(format!(".clipcat-update-{}", std::process::id()));
    std::fs::create_dir(&stage)?;
    let candidate = stage.join("new");
    let backup = stage.join("old");
    let mut preserve_backup = false;
    let result = (|| {
        let copied = command("/usr/bin/ditto").arg(source).arg(&candidate).status()?;
        if !copied.success() { return Err(std::io::Error::other("update staging failed")); }
        std::fs::rename(installed, &backup)?;
        if let Err(err) = std::fs::rename(&candidate, installed) {
            // If rollback itself fails, leave the backup on disk for recovery.
            if let Err(rollback) = std::fs::rename(&backup, installed) {
                preserve_backup = true;
                return Err(rollback);
            }
            return Err(err);
        }
        Ok(())
    })();
    if !preserve_backup { let _ = std::fs::remove_dir_all(&stage); }
    result
}

#[cfg(target_os = "macos")]
fn install(version: &str) -> Option<()> {
    let exe = std::env::current_exe().ok()?;
    let dir = std::env::temp_dir().join(format!("clipcat-download-{}", std::process::id()));
    std::fs::create_dir(&dir).ok()?;
    let result = (|| {
        let archive = dir.join("update.zip");
        let url = format!("{REPO_URL}/releases/download/v{version}/{MACOS_ASSET}");
        curl(&["-fsSL", "--max-time", "600", "-o", archive.to_str()?, &url])?;
        command("/usr/bin/ditto").args(["-x", "-k"]).arg(&archive).arg(&dir)
            .status().ok()?.success().then_some(())?;
        let app = dir.join("ClipCat.app");
        install_macos_app(&app, &exe, version)?;
        *RESTART_EXE.lock().ok()? = Some(exe.clone());
        Some(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[cfg(target_os = "macos")]
fn install_macos_app(app: &std::path::Path, exe: &std::path::Path, version: &str) -> Option<()> {
    let binary = app.join("Contents/MacOS/clipcat");
    // Check the signed bundle, requested version and CPU before touching
    // the working install. No privileged helper or permission escalation.
    command("/usr/bin/codesign").args(["--verify", "--deep", "--strict"]).arg(app)
        .status().ok()?.success().then_some(())?;
    let info = command("/usr/libexec/PlistBuddy").args(["-c", "Print :CFBundleShortVersionString"])
        .arg(app.join("Contents/Info.plist")).output().ok()?;
    (info.status.success() && String::from_utf8_lossy(&info.stdout).trim() == version).then_some(())?;
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" };
    command("/usr/bin/lipo").args(["-verify_arch", arch]).arg(&binary)
        .status().ok()?.success().then_some(())?;
    let (source, destination) = if let Some(bundle) = macos_bundle(exe) {
        (app, bundle)
    } else { (binary.as_path(), exe) };
    replace_macos_path(source, destination).ok()
}


/// Schedule a detached relaunch only once this PID has actually exited.
#[cfg(target_os = "macos")]
pub fn restart() -> bool {
    use std::process::Stdio;
    let Some(exe) = RESTART_EXE.lock().ok().and_then(|mut path| path.take()) else { return false; };
    // Positional arguments keep paths (including quotes/spaces) out of shell code.
    let script = if macos_bundle(&exe).is_some() {
        "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; exec /usr/bin/open -n \"$2\""
    } else {
        "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; exec \"$2\""
    };
    command("/bin/sh").args(["-c", script, "clipcat-restart", &std::process::id().to_string()])
        .arg(macos_bundle(&exe).unwrap_or(&exe))
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
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

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_update_replaces_binary_and_preserves_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("clipcat-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let installed = dir.join("clipcat");
        let candidate = dir.join("candidate");
        std::fs::write(&installed, b"old executable").unwrap();
        std::fs::write(&candidate, b"new executable").unwrap();
        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o755)).unwrap();
        replace_macos_path(&candidate, &installed).unwrap();
        assert_eq!(std::fs::read(&installed).unwrap(), b"new executable");
        assert_ne!(std::fs::metadata(&installed).unwrap().permissions().mode() & 0o111, 0);
        // A failed replacement must leave the working install intact.
        assert!(replace_macos_path(&dir.join("missing"), &installed).is_err());
        assert_eq!(std::fs::read(&installed).unwrap(), b"new executable");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_update_validates_signed_bundle_version_before_install() {
        let dir = std::env::temp_dir().join(format!("clipcat-bundle-test-{}", std::process::id()));
        let app = dir.join("download/ClipCat.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::copy(std::env::current_exe().unwrap(), app.join("Contents/MacOS/clipcat")).unwrap();
        std::fs::write(app.join("Contents/Info.plist"), br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.clipcat.update-test</string>
<key>CFBundleExecutable</key><string>clipcat</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>2.6.0</string>
</dict></plist>"#).unwrap();
        assert!(command("/usr/bin/codesign").args(["--force", "--sign", "-"])
            .arg(&app).output().unwrap().status.success());
        let expected = std::fs::read(app.join("Contents/MacOS/clipcat")).unwrap();
        let installed = dir.join("installed/ClipCat.app");
        std::fs::create_dir_all(installed.join("Contents/MacOS")).unwrap();
        let exe = installed.join("Contents/MacOS/clipcat");
        std::fs::write(&exe, b"working old version").unwrap();
        assert!(install_macos_app(&app, &exe, "2.7.0").is_none());
        assert_eq!(std::fs::read(&exe).unwrap(), b"working old version");
        install_macos_app(&app, &exe, "2.6.0").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), expected);
        assert!(command("/usr/bin/codesign").args(["--verify", "--deep", "--strict"])
            .arg(&installed).output().unwrap().status.success());
        std::fs::write(app.join("Contents/MacOS/clipcat"), b"broken download").unwrap();
        assert!(install_macos_app(&app, &exe, "2.6.0").is_none());
        assert_eq!(std::fs::read(&exe).unwrap(), expected);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_update_identifies_only_the_executables_own_bundle() {
        use std::path::Path;
        assert_eq!(macos_bundle(Path::new("/Applications/ClipCat.app/Contents/MacOS/clipcat")),
            Some(Path::new("/Applications/ClipCat.app")));
        assert_eq!(macos_bundle(Path::new("/tmp/Other.app/tools/clipcat")), None);
        assert_eq!(macos_bundle(Path::new("/tmp/clipcat")), None);
    }

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
