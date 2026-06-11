//! Persistent state: lifetime stats, daily stats, settings.
//! Saved as JSON under %APPDATA%\DeskCat\state.json.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    // lifetime
    pub total_keys: u64,
    pub total_clicks: u64,
    pub total_xp: u64,
    // daily
    pub today: String,
    pub keys_today: u64,
    pub clicks_today: u64,
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
            total_keys: 0,
            total_clicks: 0,
            total_xp: 0,
            today: String::new(),
            keys_today: 0,
            clicks_today: 0,
            active_min_today: 0,
        }
    }
}

/// Per-user config directory, following each platform's convention:
/// Windows `%APPDATA%\DeskCat`, macOS `~/Library/Application Support/DeskCat`,
/// Linux `$XDG_CONFIG_HOME/DeskCat` (or `~/.config/DeskCat`).
fn state_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("DeskCat"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/DeskCat"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("DeskCat"))
    }
}

fn state_file() -> Option<PathBuf> {
    state_dir().map(|d| d.join("state.json"))
}

impl Persist {
    pub fn load() -> Persist {
        let mut st: Persist = state_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let today = today_string();
        if st.today != today {
            st.today = today;
            st.keys_today = 0;
            st.clicks_today = 0;
            st.active_min_today = 0;
        }
        st
    }

    pub fn save(&self) {
        if let (Some(dir), Some(file)) = (state_dir(), state_file()) {
            let _ = std::fs::create_dir_all(&dir);
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(file, json);
            }
        }
    }

    /// Resets daily counters if the local date rolled over.
    pub fn roll_day(&mut self) -> bool {
        let today = today_string();
        if self.today != today {
            self.today = today;
            self.keys_today = 0;
            self.clicks_today = 0;
            self.active_min_today = 0;
            true
        } else {
            false
        }
    }
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
    pub name_kr: &'static str,
    pub name_en: &'static str,
}

/// Index in this array + 1 == Persist::accessory id (0 = none).
pub const ACCESSORIES: [AccessoryDef; 6] = [
    AccessoryDef { level: 2, name_kr: "빨간 목도리", name_en: "RED SCARF" },
    AccessoryDef { level: 3, name_kr: "동그란 안경", name_en: "GLASSES" },
    AccessoryDef { level: 5, name_kr: "파란 비니", name_en: "BLUE BEANIE" },
    AccessoryDef { level: 7, name_kr: "헤드폰", name_en: "HEADPHONES" },
    AccessoryDef { level: 10, name_kr: "황금 왕관", name_en: "GOLD CROWN" },
    AccessoryDef { level: 15, name_kr: "마법사 모자", name_en: "WIZARD HAT" },
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
            assert!(!acc.name_en.is_empty());
        }
    }

    #[test]
    fn today_string_is_well_formed() {
        let s = today_string();
        assert_eq!(s.len(), 10, "YYYY-MM-DD");
        assert_eq!(s.as_bytes()[4], b'-');
        assert_eq!(s.as_bytes()[7], b'-');
    }
}
