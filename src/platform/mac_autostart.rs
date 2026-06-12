//! macOS "run at login" via a per-user LaunchAgent.
//!
//! ClipCat ships as a bare binary (not a `.app` bundle), so the realistic
//! mechanism is a `~/Library/LaunchAgents/<label>.plist` with `RunAtLoad`
//! pointing at the current executable. `is_enabled` is "does that plist
//! exist", `set` writes or removes it.
//!
//! Caveat: the plist pins the absolute exe path captured when it was written;
//! moving/renaming the binary breaks it until the user toggles again. This
//! mirrors the Windows HKCU\Run value, which also stores an absolute path.

use std::path::PathBuf;

/// Reverse-DNS label; also the plist file name.
const LABEL: &str = "io.github.buildon99x.clipcat";

fn plist_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut p = PathBuf::from(home);
    p.push("Library/LaunchAgents");
    p.push(format!("{LABEL}.plist"));
    Some(p)
}

/// True when the LaunchAgent plist exists.
pub fn is_enabled() -> bool {
    plist_path().map(|p| p.exists()).unwrap_or(false)
}

/// Writes (or removes) the LaunchAgent. Best-effort: returns the resulting
/// enabled state so the caller can reflect reality even if the write failed.
pub fn set(on: bool) -> bool {
    let Some(path) = plist_path() else {
        return false;
    };
    if !on {
        let _ = std::fs::remove_file(&path);
        return is_enabled();
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let exe = exe.to_string_lossy();
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \t<key>Label</key>\n\t<string>{LABEL}</string>\n\
         \t<key>ProgramArguments</key>\n\t<array>\n\t\t<string>{exe}</string>\n\t</array>\n\
         \t<key>RunAtLoad</key>\n\t<true/>\n\
         </dict>\n</plist>\n"
    );
    let _ = std::fs::write(&path, body);
    is_enabled()
}
