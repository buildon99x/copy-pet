//! Persistent state: lifetime stats, daily stats, settings.
//! Saved as JSON under %APPDATA%\ClipCat\state.json (clip history lives in
//! clips.json beside it — see [`crate::clipboard`]). A pre-2.0 DeskCat
//! config dir is migrated on first launch.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n::{Lang, Msg};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Persist {
    // window
    pub pos_x: i32,
    pub pos_y: i32,
    pub has_pos: bool,
    pub scale_idx: usize, // 0 small, 1 normal, 2 large
    // settings
    pub accessory: usize,  // 0 = none, 1..=ACCESSORIES.len()
    pub sound_mode: u8,    // 0 off, 1 events only, 2 taps + events
    pub bubble_pinned: bool,
    pub locked: bool,
    /// Window stacking: 0 = always on top (default), 1 = normal (can go behind
    /// other windows), 2 = hidden (restored from the tray or the panel hotkey).
    /// Surfaced on Windows (tray) and macOS (tray); Linux keeps always-on-top.
    pub window_level: u8,
    /// "en" / "ko"; empty means "not chosen yet" -> detected from the OS.
    pub lang: String,
    /// When false, copy events are ignored (privacy pause).
    pub clip_capture: bool,
    /// Global panel hotkey spec, e.g. "win+shift+v" (see [`crate::hotkey`]).
    /// Editable in state.json; invalid values reset to the default on load.
    pub hotkey: String,
    /// When false, the daily GitHub release check is skipped entirely
    /// (see [`crate::update`], ADR-0009).
    pub auto_update: bool,
    // clipboard panel card (canvas units; see crate::panel::Layout)
    pub panel_w: f32,
    pub panel_h: f32,
    /// Card top-left relative to the cat's top-left — the panel can be
    /// moved independently of the cat (dragged by its header).
    pub panel_off_x: f32,
    pub panel_off_y: f32,
    /// When true (default), picking a clip closes the panel for pasting.
    pub panel_autoclose: bool,
    /// True once the user has opened the clipboard panel at least once. Gates
    /// the first-run under-pet hotkey hint, which shows only until then.
    pub onboarded: bool,
    /// When true, picking a clip also pastes it into the previously focused app
    /// (synthesized Ctrl/Cmd+V). Off by default — some users want copy-only.
    pub paste_on_select: bool,
    /// Panel list style: 0 = compact list, 1 = roomier rounded-box
    /// "thumbnail" cards that show more of each clip (default).
    pub panel_view: u8,
    // lifetime
    pub total_keys: u64,
    pub total_clicks: u64,
    pub total_copies: u64,
    pub total_xp: u64,
    // daily
    pub today: String,
    pub keys_today: u64,
    pub clicks_today: u64,
    pub copies_today: u64,
    pub active_min_today: u32,
}

impl Default for Persist {
    fn default() -> Self {
        Persist {
            pos_x: 0,
            pos_y: 0,
            has_pos: false,
            scale_idx: 1,
            accessory: 0,
            sound_mode: 1,
            bubble_pinned: false,
            locked: false,
            window_level: 0,
            lang: String::new(),
            clip_capture: true,
            hotkey: crate::hotkey::DEFAULT.to_string(),
            auto_update: true,
            panel_w: crate::panel::DEFAULT_W,
            panel_h: crate::panel::DEFAULT_H,
            panel_off_x: crate::panel::DEFAULT_OFF.0,
            panel_off_y: crate::panel::DEFAULT_OFF.1,
            panel_autoclose: true,
            onboarded: false,
            paste_on_select: false,
            panel_view: 1,
            total_keys: 0,
            total_clicks: 0,
            total_copies: 0,
            total_xp: 0,
            today: String::new(),
            keys_today: 0,
            clicks_today: 0,
            copies_today: 0,
            active_min_today: 0,
        }
    }
}

/// Per-user config directory, following each platform's convention:
/// Windows `%APPDATA%\ClipCat`, macOS `~/Library/Application Support/ClipCat`,
/// Linux `$XDG_CONFIG_HOME/ClipCat` (or `~/.config/ClipCat`).
fn dir_named(name: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(name))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support").join(name))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join(name))
    }
}

pub fn config_dir() -> Option<PathBuf> {
    dir_named("ClipCat")
}

/// Writes a config file atomically (temp file in the same dir + rename) so a
/// crash or power loss mid-write never corrupts the previous contents.
/// Shared by `state.json` and `clips.json` (see [`crate::clipboard`]).
pub(crate) fn write_atomic(path: &std::path::Path, contents: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, contents).is_ok() && std::fs::rename(&tmp, path).is_err() {
        // rename failed: don't leave the temp file orphaned in the config dir
        let _ = std::fs::remove_file(&tmp);
    }
}

fn state_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join("state.json"))
}

/// One-time migration of a pre-2.0 "DeskCat" config dir: copies the files
/// into the ClipCat dir if the new one doesn't exist yet.
fn migrate_legacy_dir() {
    let (Some(old), Some(new)) = (dir_named("DeskCat"), config_dir()) else {
        return;
    };
    if new.join("state.json").exists() || !old.join("state.json").exists() {
        return;
    }
    let _ = std::fs::create_dir_all(&new);
    for f in ["state.json", "clips.json"] {
        if old.join(f).exists() {
            let _ = std::fs::copy(old.join(f), new.join(f));
        }
    }
}

impl Persist {
    pub fn load() -> Persist {
        migrate_legacy_dir();
        let mut st: Persist = state_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let today = today_string();
        if st.today != today {
            st.reset_daily(today);
        }
        if Lang::from_code(&st.lang).is_none() {
            st.lang = detect_lang().code().to_string();
        }
        if crate::hotkey::Hotkey::parse(&st.hotkey).is_none() {
            st.hotkey = crate::hotkey::DEFAULT.to_string();
        }
        // hand-edited or corrupt panel geometry must never produce an
        // absurd or NaN layout
        (st.panel_w, st.panel_h, st.panel_off_x, st.panel_off_y) = crate::panel::clamp_geometry(
            st.panel_w,
            st.panel_h,
            st.panel_off_x,
            st.panel_off_y,
        );
        st
    }

    pub fn save(&self) {
        if let (Some(file), Ok(json)) = (state_file(), serde_json::to_string_pretty(self)) {
            write_atomic(&file, &json);
        }
    }

    pub fn lang(&self) -> Lang {
        Lang::from_code(&self.lang).unwrap_or(Lang::En)
    }

    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang.code().to_string();
    }

    fn reset_daily(&mut self, today: String) {
        self.today = today;
        self.keys_today = 0;
        self.clicks_today = 0;
        self.copies_today = 0;
        self.active_min_today = 0;
    }

    /// Resets daily counters if the local date rolled over.
    pub fn roll_day(&mut self) -> bool {
        let today = today_string();
        if self.today != today {
            self.reset_daily(today);
            true
        } else {
            false
        }
    }
}

/// Default UI language from the OS (Korean locale -> Korean, else English).
/// Like `today_string`, this is a tiny per-OS leaf the core may call.
#[cfg(windows)]
pub fn detect_lang() -> Lang {
    use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
    const LANG_KOREAN: u16 = 0x12;
    let id = unsafe { GetUserDefaultUILanguage() };
    if id & 0x3FF == LANG_KOREAN {
        Lang::Ko
    } else {
        Lang::En
    }
}

#[cfg(not(windows))]
pub fn detect_lang() -> Lang {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return if v.starts_with("ko") { Lang::Ko } else { Lang::En };
            }
        }
    }
    Lang::En
}

/// Local date as "YYYY-MM-DD". `std` has no local-time support, so each
/// platform uses its native call (Win32 `GetLocalTime`, libc `localtime_r`).
#[cfg(windows)]
pub fn today_string() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut st = unsafe { std::mem::zeroed::<windows_sys::Win32::Foundation::SYSTEMTIME>() };
    unsafe { GetLocalTime(&mut st) };
    format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
}

#[cfg(unix)]
pub fn today_string() -> String {
    // SAFETY: localtime_r writes into our stack `tm`; time() takes a null arg.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return "1970-01-01".to_string();
        }
        format!(
            "{:04}-{:02}-{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday
        )
    }
}

// ---- progression ----------------------------------------------------------

/// XP needed to go from `level` to `level + 1`.
pub fn xp_to_next(level: u32) -> u64 {
    200 + 80 * (level as u64) * (level as u64)
}

/// (current level, xp into level, xp needed for next level)
pub fn level_progress(total_xp: u64) -> (u32, u64, u64) {
    let mut level = 1u32;
    let mut rem = total_xp;
    loop {
        let need = xp_to_next(level);
        if rem < need || level >= 99 {
            return (level, rem.min(need), need);
        }
        rem -= need;
        level += 1;
    }
}

pub struct AccessoryDef {
    pub level: u32,
    pub name: Msg,
}

impl AccessoryDef {
    pub fn name(&self, lang: Lang) -> &'static str {
        crate::i18n::t(lang, self.name)
    }
}

/// Index in this array + 1 == Persist::accessory id (0 = none).
pub const ACCESSORIES: [AccessoryDef; 6] = [
    AccessoryDef { level: 2,  name: Msg::AccRedScarf },
    AccessoryDef { level: 3,  name: Msg::AccGlasses },
    AccessoryDef { level: 5,  name: Msg::AccBlueBeanie },
    AccessoryDef { level: 7,  name: Msg::AccHeadphones },
    AccessoryDef { level: 10, name: Msg::AccGoldCrown },
    AccessoryDef { level: 15, name: Msg::AccWizardHat },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_advance_monotonically() {
        // each unlock threshold should produce exactly its level
        let mut xp = 0u64;
        let mut prev = 1;
        for lv in 1..20u32 {
            let (got, into, need) = level_progress(xp);
            assert!(got >= prev, "level must not decrease");
            assert!(into <= need);
            prev = got;
            xp += xp_to_next(lv);
            let (after, _, _) = level_progress(xp);
            assert_eq!(after, lv + 1, "crossing a threshold advances one level");
        }
    }

    #[test]
    fn level_is_clamped() {
        let (lv, _, _) = level_progress(u64::MAX);
        assert!(lv <= 99);
    }

    #[test]
    fn every_accessory_has_a_reachable_level() {
        for acc in ACCESSORIES.iter() {
            assert!(acc.level >= 2 && acc.level <= 99);
            assert!(!acc.name(crate::i18n::Lang::En).is_empty());
            assert!(!acc.name(crate::i18n::Lang::Ko).is_empty());
        }
    }

    #[test]
    fn today_string_is_well_formed() {
        let s = today_string();
        assert_eq!(s.len(), 10, "YYYY-MM-DD");
        assert_eq!(s.as_bytes()[4], b'-');
        assert_eq!(s.as_bytes()[7], b'-');
    }

    #[test]
    fn old_state_json_still_deserializes() {
        // pre-2.0 state.json (no lang / clip fields) must load via defaults
        let old = r#"{"pos_x":10,"pos_y":20,"has_pos":true,"scale_idx":1,
            "accessory":2,"sound_mode":1,"bubble_pinned":false,"locked":false,
            "total_keys":5000,"total_clicks":900,"total_xp":12000,
            "today":"2025-01-01","keys_today":1,"clicks_today":2,
            "active_min_today":3}"#;
        let st: Persist = serde_json::from_str(old).unwrap();
        assert_eq!(st.total_keys, 5000);
        assert!(st.clip_capture, "clip capture defaults on");
        assert_eq!(st.total_copies, 0);
        assert!(st.lang.is_empty());
        assert_eq!(st.hotkey, crate::hotkey::DEFAULT, "hotkey defaults in");
        assert!(st.auto_update, "update check defaults on");
        assert!(st.panel_autoclose, "panel closes after copy by default");
        assert_eq!(st.panel_view, 1, "panel opens in card view by default");
        assert_eq!(st.panel_w, crate::panel::DEFAULT_W, "panel size defaults in");
        assert_eq!(
            (st.panel_off_x, st.panel_off_y),
            crate::panel::DEFAULT_OFF,
            "panel offset defaults in"
        );
    }
}
