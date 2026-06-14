//! The global clipboard-panel hotkey: parsing and display formatting of the
//! persisted spec string (e.g. `"win+shift+v"`, see [`crate::state::Persist`]).
//! Platform-agnostic — actually detecting the key combination is the
//! backend's job (`RegisterHotKey` in `platform/windows.rs`; a chord matcher
//! on the rdev listener in `platform/portable.rs`, see ADR-0008).
//!
//! The `win` modifier means the OS "super" key: the Windows key, ⌘ Command
//! on macOS, Super on Linux — so the one default spec is Win+Shift+V on
//! Windows and Cmd+Shift+V on macOS.

/// Default panel hotkey: Win+Shift+V (Cmd+Shift+V on macOS).
pub const DEFAULT: &str = "win+shift+v";
/// Fallback when the default cannot be registered (clash with another app).
pub const FALLBACK: &str = "ctrl+shift+v";

/// Safe, always-parseable presets the settings menu cycles through (instead of
/// a free-form rebind UI). The default leads, so cycling from a fresh install
/// advances predictably. Every entry must parse via [`Hotkey::parse`].
pub const PRESETS: &[&str] = &["win+shift+v", "ctrl+shift+v", "alt+shift+v", "ctrl+shift+c"];

/// The preset after `current` (wrapping). Comparison is on the parsed chord, so
/// case/spacing don't matter; a spec that isn't a preset (e.g. a hand-edited
/// custom chord) cycles to the first preset.
pub fn next_preset(current: &str) -> &'static str {
    let cur = Hotkey::from_spec(current);
    match PRESETS.iter().position(|p| Hotkey::from_spec(p) == cur) {
        Some(i) => PRESETS[(i + 1) % PRESETS.len()],
        None => PRESETS[0],
    }
}

/// What the `win` modifier is called on this OS (display only).
pub fn super_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "CMD"
    } else if cfg!(windows) {
        "WIN"
    } else {
        "SUPER"
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hotkey {
    pub win: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Uppercase `A..=Z` or `0..=9` (which equals the Win32 virtual-key code).
    pub key: char,
}

impl Hotkey {
    /// Parses `"win+shift+v"`-style specs: any of the modifiers
    /// win/ctrl/shift/alt (at least one) plus one A-Z/0-9 key, joined by
    /// `+`, case- and whitespace-insensitive.
    pub fn parse(spec: &str) -> Option<Hotkey> {
        let mut hk = Hotkey {
            win: false,
            ctrl: false,
            shift: false,
            alt: false,
            key: '\0',
        };
        for part in spec.split('+') {
            match part.trim().to_lowercase().as_str() {
                "win" | "windows" | "super" | "meta" | "cmd" => hk.win = true,
                "ctrl" | "control" => hk.ctrl = true,
                "shift" => hk.shift = true,
                "alt" => hk.alt = true,
                k => {
                    let mut chars = k.chars();
                    let (Some(c), None) = (chars.next(), chars.next()) else {
                        return None;
                    };
                    if !c.is_ascii_alphanumeric() || hk.key != '\0' {
                        return None;
                    }
                    hk.key = c.to_ascii_uppercase();
                }
            }
        }
        let has_mod = hk.win || hk.ctrl || hk.shift || hk.alt;
        (has_mod && hk.key != '\0').then_some(hk)
    }

    /// `spec`, or the default when it does not parse (bad hand-edited file).
    pub fn from_spec(spec: &str) -> Hotkey {
        Hotkey::parse(spec)
            .or_else(|| Hotkey::parse(DEFAULT))
            .expect("DEFAULT must parse")
    }

    /// Human-readable label (panel footer, tray menu): `"WIN+SHIFT+V"` on
    /// Windows, `"CMD+SHIFT+V"` on macOS, `"SUPER+SHIFT+V"` on Linux.
    pub fn display(&self) -> String {
        let mut out = String::new();
        for (on, name) in [
            (self.win, super_name()),
            (self.ctrl, "CTRL"),
            (self.alt, "ALT"),
            (self.shift, "SHIFT"),
        ] {
            if on {
                out.push_str(name);
                out.push('+');
            }
        }
        out.push(self.key);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_super_shift_v() {
        let hk = Hotkey::from_spec(DEFAULT);
        assert!(hk.win && hk.shift && !hk.ctrl && !hk.alt);
        assert_eq!(hk.key, 'V');
        // the super key is named per OS: WIN / CMD / SUPER
        assert_eq!(hk.display(), format!("{}+SHIFT+V", super_name()));
        if cfg!(target_os = "macos") {
            assert_eq!(super_name(), "CMD");
        }
    }

    #[test]
    fn fallback_is_ctrl_shift_v() {
        let hk = Hotkey::from_spec(FALLBACK);
        assert!(hk.ctrl && hk.shift && !hk.win);
        assert_eq!(hk.display(), "CTRL+SHIFT+V");
    }

    #[test]
    fn parse_is_forgiving_about_case_and_spaces() {
        let hk = Hotkey::parse(" Super + Shift + b ").unwrap();
        assert!(hk.win && hk.shift);
        assert_eq!(hk.key, 'B');
        assert_eq!(Hotkey::parse("ctrl+alt+9").unwrap().key, '9');
    }

    #[test]
    fn presets_all_parse_and_display() {
        for p in PRESETS {
            let hk = Hotkey::parse(p).unwrap_or_else(|| panic!("preset {p:?} must parse"));
            assert!(!hk.display().is_empty());
        }
        assert_eq!(PRESETS[0], DEFAULT, "the default leads the cycle");
    }

    #[test]
    fn next_preset_cycles_and_wraps() {
        // walk the whole ring and return to the start
        let mut spec = PRESETS[0].to_string();
        for expected in PRESETS.iter().skip(1).chain(std::iter::once(&PRESETS[0])) {
            spec = next_preset(&spec).to_string();
            assert_eq!(&spec.as_str(), expected);
        }
        // comparison is on the parsed chord, not the string spelling
        assert_eq!(next_preset("Win+Shift+V"), PRESETS[1]);
        // an unknown/custom chord jumps to the first preset
        assert_eq!(next_preset("ctrl+alt+k"), PRESETS[0]);
    }

    #[test]
    fn rejects_invalid_specs() {
        for bad in ["", "v", "shift", "ctrl+", "ctrl+vv", "ctrl+ä", "ctrl+s+v"] {
            assert_eq!(Hotkey::parse(bad), None, "{bad:?} must not parse");
        }
        // ...and from_spec falls back to the default instead
        assert_eq!(Hotkey::from_spec("garbage"), Hotkey::from_spec(DEFAULT));
    }
}
