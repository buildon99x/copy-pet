//! Dark-premium design tokens — the single source of truth for the restyle.
//!
//! These are transcribed from `docs/design/tokens/{colors,radius_spacing,
//! typography}.json` (the imported design package) into Rust constants in the
//! `(r, g, b, a)` tuple form the renderer already uses (see [`crate::render`]).
//! Colors given as `#RRGGBB` in the package are alpha `255` here; the two
//! `#FFFFFFxx` border tokens carry their JSON alpha. They are consumed
//! progressively by the visual milestones (pet visuals, panel shell, rows,
//! FX); keeping the whole palette here means a value is defined once and the
//! draw code never hand-rolls a hex literal.
//!
//! Privacy/asset rules are unaffected: tokens are plain numbers compiled into
//! the binary — no files, no fonts, no network.

#![allow(dead_code)] // palette is filled in ahead of the milestones that draw with it

pub type Rgba = (u8, u8, u8, u8);

// ---- surfaces & background -------------------------------------------------
pub const BG_DESKTOP_SHADOW: Rgba = (0x05, 0x07, 0x0C, 0xFF);
pub const SURFACE_WINDOW: Rgba = (0x0F, 0x12, 0x18, 0xFF);
pub const SURFACE_PANEL: Rgba = (0x18, 0x1C, 0x24, 0xFF);
pub const SURFACE_CARD: Rgba = (0x20, 0x26, 0x33, 0xFF);
pub const SURFACE_CONTROL: Rgba = (0x22, 0x27, 0x32, 0xFF);
pub const SURFACE_CONTROL_HOVER: Rgba = (0x2A, 0x31, 0x40, 0xFF);
pub const SURFACE_CONTROL_ACTIVE: Rgba = (0x34, 0x3D, 0x50, 0xFF);

// ---- borders ---------------------------------------------------------------
pub const BORDER_SUBTLE: Rgba = (0xFF, 0xFF, 0xFF, 0x1A); // white @ ~10%
pub const BORDER_STRONG: Rgba = (0xFF, 0xFF, 0xFF, 0x2E); // white @ ~18%
pub const BORDER_FOCUS: Rgba = (0xFF, 0xCB, 0x36, 0xFF); // gold focus ring

// ---- text ------------------------------------------------------------------
pub const TEXT_PRIMARY: Rgba = (0xF5, 0xF7, 0xFA, 0xFF);
pub const TEXT_SECONDARY: Rgba = (0xB8, 0xC0, 0xCC, 0xFF);
pub const TEXT_MUTED: Rgba = (0x77, 0x82, 0x92, 0xFF);

// ---- accents ---------------------------------------------------------------
pub const ACCENT_GOLD: Rgba = (0xFF, 0xCB, 0x36, 0xFF);
pub const ACCENT_GOLD_2: Rgba = (0xF3, 0xA9, 0x1B, 0xFF);
pub const ACCENT_GREEN: Rgba = (0x6E, 0xE7, 0x79, 0xFF);
pub const ACCENT_RED: Rgba = (0xFF, 0x64, 0x69, 0xFF);
pub const ACCENT_BLUE: Rgba = (0x3A, 0xA1, 0xFF, 0xFF);
pub const ACCENT_PURPLE: Rgba = (0x8F, 0x7C, 0xF6, 0xFF);

// ---- mascot (cat / fish) ---------------------------------------------------
pub const CAT_FUR: Rgba = (0xFF, 0xF3, 0xE2, 0xFF);
pub const CAT_LINE: Rgba = (0x5A, 0x3E, 0x30, 0xFF);
pub const CAT_INNER_EAR: Rgba = (0xFF, 0xD5, 0xCF, 0xFF);
pub const FISH_BODY: Rgba = (0xE8, 0xC4, 0x8F, 0xFF);
pub const FISH_FIN: Rgba = (0xB9, 0x8F, 0x65, 0xFF);

// ---- radii (px) ------------------------------------------------------------
pub const RADIUS_XS: f32 = 6.0;
pub const RADIUS_SM: f32 = 8.0;
pub const RADIUS_MD: f32 = 12.0;
pub const RADIUS_LG: f32 = 16.0;
pub const RADIUS_XL: f32 = 22.0;
/// Sentinel "fully rounded" radius; clamp to half the shorter side at use.
pub const RADIUS_PILL: f32 = 999.0;

// ---- spacing scale (px) ----------------------------------------------------
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 20.0;
pub const SPACE_6: f32 = 24.0;
pub const SPACE_8: f32 = 32.0;

// ---- stroke widths (px) ----------------------------------------------------
pub const STROKE_HAIRLINE: f32 = 1.0;
pub const STROKE_NORMAL: f32 = 1.5;
pub const STROKE_FOCUS: f32 = 2.0;

// ---- type scale (px) — OS UI font + Hangul fallback, never bundled ---------
pub const TYPE_CAPTION: f32 = 11.0;
pub const TYPE_BODY: f32 = 13.0;
pub const TYPE_BODY_STRONG: f32 = 14.0;
pub const TYPE_TITLE: f32 = 16.0;
pub const TYPE_HERO: f32 = 28.0;

/// Scale a token color's alpha by `a` (0..=1), preserving rgb. Mirrors the
/// renderer's local `fade` so token surfaces can be drawn at partial opacity.
pub const fn with_alpha(c: Rgba, a: u8) -> Rgba {
    (c.0, c.1, c.2, a)
}

// ---- runtime light/dark theme ----------------------------------------------
//
// The surface/border/text tokens are the only ones that flip between a dark
// and a light tone; accents and the mascot keep their hue on both (gold/green/
// red/blue/purple read on either background). The renderer reads these via the
// accessor fns below instead of the raw consts, so a single global switch
// re-tones the whole UI. The switch is a plain atomic — no OS, no files, no
// network — set once per frame from the user's persisted choice
// (`Persist::theme`). The dark `Palette` is built from the consts above so the
// premium dark tone stays the single source of truth.

use std::sync::atomic::{AtomicU8, Ordering};

/// The two color tones the user can pick (settings menu).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    /// Stable code persisted in `state.json` (parity with `Lang::code`).
    pub fn code(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }
    /// Parse a persisted code; unknown/empty falls back to the default (dark).
    pub fn from_code(s: &str) -> Theme {
        match s {
            "light" => Theme::Light,
            _ => Theme::Dark,
        }
    }
}

/// The themed half of the palette — every token whose tone flips with the
/// theme. Accents/mascot/radii/spacing stay as the consts above.
pub struct Palette {
    pub surface_window: Rgba,
    pub surface_panel: Rgba,
    pub surface_card: Rgba,
    pub surface_control: Rgba,
    pub surface_control_hover: Rgba,
    pub surface_control_active: Rgba,
    pub border_subtle: Rgba,
    pub border_strong: Rgba,
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
}

/// Dark-premium tone (default) — assembled from the source-of-truth consts.
pub const DARK: Palette = Palette {
    surface_window: SURFACE_WINDOW,
    surface_panel: SURFACE_PANEL,
    surface_card: SURFACE_CARD,
    surface_control: SURFACE_CONTROL,
    surface_control_hover: SURFACE_CONTROL_HOVER,
    surface_control_active: SURFACE_CONTROL_ACTIVE,
    border_subtle: BORDER_SUBTLE,
    border_strong: BORDER_STRONG,
    text_primary: TEXT_PRIMARY,
    text_secondary: TEXT_SECONDARY,
    text_muted: TEXT_MUTED,
};

/// Light tone: soft off-white page, white elevated cards, near-black text and
/// black-tinted hairline borders (the dark theme's white borders inverted).
pub const LIGHT: Palette = Palette {
    surface_window: (0xE9, 0xEC, 0xF1, 0xFF),
    surface_panel: (0xFF, 0xFF, 0xFF, 0xFF),
    surface_card: (0xF1, 0xF3, 0xF7, 0xFF),
    surface_control: (0xEC, 0xEF, 0xF3, 0xFF),
    surface_control_hover: (0xE2, 0xE6, 0xED, 0xFF),
    surface_control_active: (0xD4, 0xDB, 0xE5, 0xFF),
    border_subtle: (0x10, 0x16, 0x20, 0x18), // black @ ~9%
    border_strong: (0x10, 0x16, 0x20, 0x2B), // black @ ~17%
    text_primary: (0x16, 0x1B, 0x24, 0xFF),
    text_secondary: (0x44, 0x4F, 0x5E, 0xFF),
    text_muted: (0x83, 0x8D, 0x9C, 0xFF),
};

static THEME: AtomicU8 = AtomicU8::new(0); // 0 = dark, 1 = light

/// Select the active tone (called once per frame from the persisted choice).
pub fn set_theme(t: Theme) {
    THEME.store(matches!(t, Theme::Light) as u8, Ordering::Relaxed);
}

/// The currently selected tone.
pub fn theme() -> Theme {
    if THEME.load(Ordering::Relaxed) == 1 {
        Theme::Light
    } else {
        Theme::Dark
    }
}

/// The active palette.
pub fn palette() -> &'static Palette {
    match theme() {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
    }
}

// Per-token accessors the renderer uses in place of the raw themed consts.
pub fn surface_window() -> Rgba { palette().surface_window }
pub fn surface_panel() -> Rgba { palette().surface_panel }
pub fn surface_card() -> Rgba { palette().surface_card }
pub fn surface_control() -> Rgba { palette().surface_control }
pub fn surface_control_hover() -> Rgba { palette().surface_control_hover }
pub fn surface_control_active() -> Rgba { palette().surface_control_active }
pub fn border_subtle() -> Rgba { palette().border_subtle }
pub fn border_strong() -> Rgba { palette().border_strong }
pub fn text_primary() -> Rgba { palette().text_primary }
pub fn text_secondary() -> Rgba { palette().text_secondary }
pub fn text_muted() -> Rgba { palette().text_muted }
