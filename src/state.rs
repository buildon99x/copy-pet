//! Persistent state: local settings.
//! Saved as JSON under %APPDATA%\ClipCat\state.json (clip history lives in
//! clips.json beside it — see [`crate::clipboard`]). A pre-2.0 DeskCat
//! config dir is migrated on first launch.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n::Lang;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Persist {
    // window
    pub pos_x: i32,
    pub pos_y: i32,
    pub has_pos: bool,
    pub scale_idx: usize, // 0 small, 1 normal, 2 large
    // settings
    #[serde(deserialize_with = "deserialize_cat_kind")]
    pub cat_kind: String, // stable character ID; old accessory fields are ignored
    pub sound_mode: u8,   // 0 off, 1 events only
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
    // Legacy statistics/growth fields are intentionally accepted as unknown
    // serde fields and omitted on the next normal save.
}

// A malformed new preference must not invalidate legacy settings.
fn deserialize_cat_kind<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(crate::cat::CatKind::from_id(value.as_str().unwrap_or("ragdoll")).id().into())
}

impl Default for Persist {
    fn default() -> Self {
        Persist {
            pos_x: 0,
            pos_y: 0,
            has_pos: false,
            scale_idx: 1,
            cat_kind: "ragdoll".into(),
            sound_mode: 1,
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
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join("Library/Application Support")
                .join(name)
        })
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
pub(crate) fn write_atomic(path: &std::path::Path, contents: &str) -> bool {
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, contents).is_err() {
        return false;
    }
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    true
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
        st.clamp_settings();
        st
    }

    /// Hardens the scalar settings against a hand-edited/corrupt state.json.
    /// An out-of-range value would leave its radio submenu with no checked
    /// item (the `current == i` scan never matches) and hand the backend an
    /// unclamped `window_level` at startup. Every read site is already
    /// defensive, so this only self-heals what the menus display — valid data
    /// is untouched. Mirrors the geometry/hotkey hardening in `load`.
    fn clamp_settings(&mut self) {
        self.scale_idx = self.scale_idx.min(crate::pet::SCALES.len() - 1);
        self.sound_mode = self.sound_mode.min(1);
        self.panel_view = self.panel_view.min(1);
        self.cat_kind = crate::cat::CatKind::from_id(&self.cat_kind).id().into();
        // window_level 2 is "Hide": saturating a corrupt value to 2 would
        // enforce Hide on startup (both backends call apply_window_level) and
        // launch the app invisible — the opposite of self-healing. Reset an
        // out-of-range level to the default visible mode (0 = always-on-top,
        // the factory default) instead of clamping to Hide.
        if self.window_level > 2 {
            self.window_level = 0;
        }
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

}

/// Default UI language from the OS (Korean locale -> Korean, else English).
/// This is a tiny per-OS leaf the core may call.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_statistics_are_discarded_on_normal_serialization() {
        let legacy = r#"{"total_keys":42,"total_clicks":12,"total_copies":9,"total_xp":279,"today":"2026-01-01","keys_today":3,"clicks_today":2,"copies_today":1,"active_min_today":5,"bubble_pinned":true,"cat_kind":"british-shorthair","locked":true,"clip_capture":false,"hotkey":"ctrl+shift+b","panel_w":420}"#;
        let state: Persist = serde_json::from_str(legacy).unwrap();
        let saved = serde_json::to_value(&state).unwrap();
        for key in ["total_keys", "total_clicks", "total_copies", "total_xp", "today", "keys_today", "clicks_today", "copies_today", "active_min_today", "bubble_pinned"] {
            assert!(saved.get(key).is_none(), "legacy field {key} must not be saved");
        }
        assert_eq!(saved["cat_kind"], "british-shorthair");
        assert_eq!(saved["locked"], true);
        assert_eq!(saved["clip_capture"], false);
        assert_eq!(saved["hotkey"], "ctrl+shift+b");
        assert_eq!(saved["panel_w"], 420.0);
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
        assert!(st.clip_capture, "clip capture defaults on");
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

    #[test]
    fn legacy_accessory_and_unknown_cat_keep_existing_settings() {
        let st: Persist =
            serde_json::from_str(r#"{"accessory":8,"total_xp":12345,"total_keys":42}"#).unwrap();
        assert_eq!(st.cat_kind, "ragdoll");
        let mut st: Persist =
            serde_json::from_str(r#"{"cat_kind":"future-cat","total_xp":12345,"sound_mode":2}"#)
                .unwrap();
        st.clamp_settings();
        assert_eq!(st.cat_kind, "ragdoll");
        assert_eq!(st.sound_mode, 1);
    }

    #[test]
    fn malformed_cat_setting_does_not_discard_existing_settings() {
        let st: Persist=serde_json::from_str(r#"{"cat_kind":42,"total_xp":9001,"total_keys":123,"locked":true,"lang":"ko"}"#).expect("only the invalid character preference falls back");
        assert_eq!(st.cat_kind,"ragdoll");
        assert!(st.locked);
        assert_eq!(st.lang, "ko");
    }

    #[test]
    fn out_of_range_settings_are_clamped_on_load() {
        // A hand-edited/corrupt state.json with out-of-range scalars must not
        // leave a radio submenu with no checked item or feed the backend an
        // unclamped window_level. clamp_settings self-heals them to valid
        // indices (mirrors the geometry/hotkey hardening in load).
        let mut st = Persist {
            scale_idx: 7,
            sound_mode: 9,
            window_level: 200,
            panel_view: 5,
            cat_kind: "unknown".into(),
            ..Persist::default()
        };
        st.clamp_settings();
        assert_eq!(st.scale_idx, crate::pet::SCALES.len() - 1);
        assert_eq!(st.sound_mode, 1);
        assert_eq!(
            st.window_level, 0,
            "out-of-range window_level resets to a visible mode, not Hide (2)"
        );
        assert_eq!(st.panel_view, 1);
        assert_eq!(st.cat_kind, "ragdoll");

        // valid data is left untouched
        let mut ok = Persist {
            scale_idx: 0,
            sound_mode: 1,
            window_level: 2,
            panel_view: 0,
            cat_kind: "ragdoll".into(),
            ..Persist::default()
        };
        ok.clamp_settings();
        assert_eq!(
            (
                ok.scale_idx,
                ok.sound_mode,
                ok.window_level,
                ok.panel_view,
                ok.cat_kind.as_str()
            ),
            (0, 1, 2, 0, "ragdoll")
        );
    }
}
