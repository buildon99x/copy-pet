//! The global clipboard-panel hotkey: parsing and display formatting of the
//! persisted spec string (e.g. `"win+shift+v"`, see [`crate::state::Persist`]).
//! Platform-agnostic — actually registering the key combination is the
//! backend's job (`RegisterHotKey` in `platform/windows.rs`; the portable
//! backend has no global hotkey because its global input listener is, by the
//! privacy rules, never allowed to inspect which key was pressed).

/// Default panel hotkey: Win+Shift+V.
pub const DEFAULT: &str = "win+shift+v";
/// Fallback when the default cannot be registered (clash with another app).
pub const FALLBACK: &str = "ctrl+shift+v";

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

    /// Human-readable label, e.g. `"WIN+SHIFT+V"` (panel footer, tray menu).
    pub fn display(&self) -> String {
        let mut out = String::new();
        for (on, name) in [
            (self.win, "WIN"),
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
    fn default_is_win_shift_v() {
        let hk = Hotkey::from_spec(DEFAULT);
        assert!(hk.win && hk.shift && !hk.ctrl && !hk.alt);
        assert_eq!(hk.key, 'V');
        assert_eq!(hk.display(), "WIN+SHIFT+V");
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
    fn rejects_invalid_specs() {
        for bad in ["", "v", "shift", "ctrl+", "ctrl+vv", "ctrl+ä", "ctrl+s+v"] {
            assert_eq!(Hotkey::parse(bad), None, "{bad:?} must not parse");
        }
        // ...and from_spec falls back to the default instead
        assert_eq!(Hotkey::from_spec("garbage"), Hotkey::from_spec(DEFAULT));
    }
}
