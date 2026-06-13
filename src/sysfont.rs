//! System-font text for everything ClipCat draws (clipboard panel, toast,
//! stats bubble, fish badge letters): loads the OS UI font (plus a
//! Hangul-capable fallback) from well-known font files at startup and
//! rasterizes it with `ab_glyph`. Nothing is bundled and nothing leaves the
//! machine — fonts are read from the local font directories only, with
//! `std::fs` (no OS API; the per-OS path tables are plain `#[cfg]` data,
//! like `state::today_string`). See ADR-0007 and ADR-0011.
//!
//! There is no built-in font anymore (the old 5x7 pixel font and the vector
//! Hangul were removed, ADR-0011): a character no loaded font covers — or
//! every character on a system without any known font file — draws as a
//! hollow "tofu" box so layout and hit-testing stay intact.
//!
//! The `px` parameter is a legacy unit kept from the old pixel font (the
//! size of one of its pixels; a text cell is 7*px tall) so the existing
//! layout grid stayed put; the system font is sized to the same optical
//! height via [`EM_PER_PX`].

use ab_glyph::{Font, FontArc, ScaleFont};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tiny_skia::{Pixmap, Rect, Transform};

/// Font pixel-height per `px` unit: a text cell at `px` is 7*px tall;
/// system fonts need ~1.30x that em size for the same optical (cap) height.
const EM_PER_PX: f32 = 9.1;

/// Advance (in `px` units) of the hollow fallback box drawn for characters
/// no loaded font covers; matches the old pixel-font cell so mixed text
/// keeps sane spacing.
const TOFU_ADV: f32 = 6.0;

// ---- font discovery ---------------------------------------------------------

/// Candidate font files, tried in order. First existing+parsable file wins
/// per slot: slot 0 is the primary UI font, slot 1 the Hangul fallback.
/// (`@idx` selects a face inside a .ttc collection.)
#[cfg(windows)]
fn candidates() -> [Vec<String>; 2] {
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let f = |n: &str| format!("{dir}\\Fonts\\{n}");
    [
        vec![f("segoeui.ttf"), f("tahoma.ttf"), f("arial.ttf")],
        vec![f("malgun.ttf"), f("gulim.ttc"), f("batang.ttc")],
    ]
}

#[cfg(target_os = "macos")]
fn candidates() -> [Vec<String>; 2] {
    let s = |n: &str| format!("/System/Library/Fonts/{n}");
    [
        vec![
            s("SFNS.ttf"),
            s("Helvetica.ttc"),
            s("Supplemental/Arial.ttf"),
        ],
        vec![
            s("AppleSDGothicNeo.ttc"),
            s("Supplemental/AppleGothic.ttf"),
            s("Supplemental/Arial Unicode.ttf"),
        ],
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn candidates() -> [Vec<String>; 2] {
    // Common locations across distros; file names are stable even when the
    // directory layout differs, so search the font roots for them.
    let latin = ["NotoSans-Regular.ttf", "DejaVuSans.ttf", "FreeSans.ttf"];
    let hangul = [
        "NotoSansCJK-Regular.ttc",
        "NotoSansCJKkr-Regular.otf",
        "NotoSansKR-Regular.ttf",
        "NanumGothic.ttf",
        "UnDotum.ttf",
    ];
    let find = |names: &[&str]| -> Vec<String> {
        let mut out = Vec::new();
        for root in font_roots() {
            for name in names {
                if let Some(p) = find_file(&root, name, 3) {
                    out.push(p.to_string_lossy().into_owned());
                }
            }
        }
        out
    };
    [find(&latin), find(&hangul)]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn font_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".fonts"));
    }
    roots
}

/// Breadth-limited search for `name` under `dir`, at most `depth` levels.
#[cfg(all(unix, not(target_os = "macos")))]
fn find_file(dir: &std::path::Path, name: &str, depth: u32) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if p.file_name().map(|f| f == name).unwrap_or(false) {
            return Some(p);
        }
    }
    subdirs.sort();
    for sub in subdirs {
        if let Some(p) = find_file(&sub, name, depth - 1) {
            return Some(p);
        }
    }
    None
}

fn load_font(path: &str) -> Option<FontArc> {
    let bytes = std::fs::read(PathBuf::from(path)).ok()?;
    // .ttc collections: take the first face (index 0 is the family default).
    FontArc::try_from_vec(bytes).ok()
}

/// The loaded system fonts: `[0]` primary UI font, `[1..]` fallbacks
/// (Hangul). Empty when nothing could be found — every character then
/// draws as a tofu box.
fn fonts() -> &'static [FontArc] {
    static FONTS: OnceLock<Vec<FontArc>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut out = Vec::new();
        for slot in candidates() {
            if let Some(font) = slot.iter().find_map(|p| load_font(p)) {
                out.push(font);
            }
        }
        out
    })
}

/// True when at least one system font was found (minimal containers and
/// exotic distros may have none; text then renders as tofu boxes).
pub fn available() -> bool {
    !fonts().is_empty()
}

/// (font index, glyph) for `c`, preferring the primary font. `None` when no
/// loaded font covers the character — it draws as a tofu box.
fn glyph_for(c: char) -> Option<(usize, ab_glyph::GlyphId)> {
    for (i, f) in fonts().iter().enumerate() {
        let id = f.glyph_id(c);
        if id.0 != 0 {
            return Some((i, id));
        }
    }
    None
}

// ---- measuring ----------------------------------------------------------------

fn advance_of(c: char, px: f32) -> f32 {
    match glyph_for(c) {
        Some((i, id)) => fonts()[i].as_scaled(px * EM_PER_PX).h_advance(id),
        None => TOFU_ADV * px,
    }
}

/// Width of `text` in canvas units at size `px`.
pub fn measure(text: &str, px: f32) -> f32 {
    text.chars().map(|c| advance_of(c, px)).sum()
}

/// Truncates `text` to fit `max_w` canvas units at size `px`, appending ".."
/// when something was cut.
pub fn truncate_to_width(text: &str, px: f32, max_w: f32) -> String {
    if measure(text, px) <= max_w {
        return text.to_string();
    }
    let ell = measure("..", px);
    let mut w = 0.0;
    let mut out = String::new();
    for c in text.chars() {
        let a = advance_of(c, px);
        if w + a + ell > max_w {
            out.push_str("..");
            return out;
        }
        out.push(c);
        w += a;
    }
    out
}

/// Splits `text` into up to two lines, each fitting `max_w` canvas units at
/// size `px`. The first line greedily takes as many characters as fit; the
/// remainder goes on the second line, truncated with ".." if it overflows.
/// The second line is empty when everything fit on the first.
pub fn wrap_two(text: &str, px: f32, max_w: f32) -> (String, String) {
    let mut l1 = String::new();
    let mut w = 0.0;
    for (i, c) in text.char_indices() {
        let a = advance_of(c, px);
        if w + a > max_w {
            return (l1, truncate_to_width(&text[i..], px, max_w));
        }
        l1.push(c);
        w += a;
    }
    (l1, String::new())
}

// ---- rasterizing -------------------------------------------------------------

/// One rasterized glyph: an alpha coverage mask plus its placement relative
/// to the (device-space) pen position and baseline.
struct Mask {
    w: u32,
    h: u32,
    left: f32,
    top: f32,
    cov: Vec<u8>,
}

type GlyphKey = (usize, u16, u32); // font index, glyph id, quantized px size

fn glyph_cache() -> &'static Mutex<HashMap<GlyphKey, Option<Mask>>> {
    static CACHE: OnceLock<Mutex<HashMap<GlyphKey, Option<Mask>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn rasterize(font_idx: usize, id: ab_glyph::GlyphId, em_dev: f32) -> Option<Mask> {
    let font = &fonts()[font_idx];
    let glyph = id.with_scale_and_position(em_dev, ab_glyph::point(0.0, 0.0));
    let outlined = font.outline_glyph(glyph)?;
    let bounds = outlined.px_bounds();
    let (w, h) = (bounds.width().ceil() as u32, bounds.height().ceil() as u32);
    if w == 0 || h == 0 {
        return None;
    }
    let mut cov = vec![0u8; (w * h) as usize];
    outlined.draw(|x, y, c| {
        if x < w && y < h {
            cov[(y * w + x) as usize] = (c * 255.0) as u8;
        }
    });
    Some(Mask {
        w,
        h,
        left: bounds.min.x,
        top: bounds.min.y,
        cov,
    })
}

/// Source-over blend of a coverage mask in `rgba` (straight alpha) onto the
/// premultiplied pixmap, at integer device coordinates.
fn blend_mask(pm: &mut Pixmap, mask: &Mask, ox: f32, oy: f32, rgba: (u8, u8, u8, u8)) {
    let (pw, ph) = (pm.width() as i64, pm.height() as i64);
    let (x0, y0) = (ox.round() as i64, oy.round() as i64);
    let a_text = rgba.3 as u32;
    let data = pm.data_mut();
    for my in 0..mask.h as i64 {
        let py = y0 + my;
        if py < 0 || py >= ph {
            continue;
        }
        for mx in 0..mask.w as i64 {
            let px = x0 + mx;
            if px < 0 || px >= pw {
                continue;
            }
            let c = mask.cov[(my * mask.w as i64 + mx) as usize] as u32;
            if c == 0 {
                continue;
            }
            let sa = c * a_text / 255; // 0..=255 effective alpha
            if sa == 0 {
                continue;
            }
            let i = ((py * pw + px) * 4) as usize;
            let inv = 255 - sa;
            // premultiplied source-over
            data[i] = ((rgba.0 as u32 * sa + data[i] as u32 * inv) / 255) as u8;
            data[i + 1] = ((rgba.1 as u32 * sa + data[i + 1] as u32 * inv) / 255) as u8;
            data[i + 2] = ((rgba.2 as u32 * sa + data[i + 2] as u32 * inv) / 255) as u8;
            data[i + 3] = (sa + data[i + 3] as u32 * inv / 255) as u8;
        }
    }
}

/// Hollow "tofu" box for a character no loaded font covers (and for every
/// character when no system font was found): four thin rects matching the
/// old 5x7 glyph cell, so something legible always renders.
fn draw_tofu(pm: &mut Pixmap, x: f32, y: f32, px: f32, rgba: (u8, u8, u8, u8), ts: Transform) {
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(rgba.0, rgba.1, rgba.2, rgba.3);
    paint.anti_alias = false;
    let (w, h, t) = (5.0 * px, 7.0 * px, px.max(0.8));
    let sides = [
        Rect::from_xywh(x, y, w, t),
        Rect::from_xywh(x, y + h - t, w, t),
        Rect::from_xywh(x, y, t, h),
        Rect::from_xywh(x + w - t, y, t, h),
    ];
    for r in sides.into_iter().flatten() {
        pm.fill_rect(r, &paint, ts, None);
    }
}

/// Draws `text` with its top-left at (x, y) in canvas units. `ts` should be
/// a scale+translate transform (which is all the UI uses); under a rotated
/// transform glyph rasterization is skipped and tofu boxes (which rotate
/// fine) are drawn instead.
pub fn draw(pm: &mut Pixmap, text: &str, x: f32, y: f32, px: f32, rgba: (u8, u8, u8, u8), ts: Transform) {
    let rotated = ts.kx != 0.0 || ts.ky != 0.0;
    let em = px * EM_PER_PX;
    let em_dev = em * ts.sx;
    // Top-aligned on the legacy 7*px text cell: center the (taller) em box
    // on that cell so all call sites share one layout grid.
    let cell_pad = (em - 7.0 * px) / 2.0;
    let mut pen = x;
    let cache = glyph_cache();
    for c in text.chars() {
        let glyph = glyph_for(c);
        let adv = match glyph {
            Some((fi, id)) => fonts()[fi].as_scaled(em).h_advance(id),
            None => TOFU_ADV * px,
        };
        match glyph {
            Some((fi, id)) if !rotated => {
                let key: GlyphKey = (fi, id.0, (em_dev * 4.0).round() as u32);
                let mut cache = cache.lock().unwrap();
                let mask = cache
                    .entry(key)
                    .or_insert_with(|| rasterize(fi, id, em_dev));
                if let Some(mask) = mask {
                    let ascent_dev = fonts()[fi].as_scaled(em_dev).ascent();
                    let dev_x = ts.tx + pen * ts.sx + mask.left;
                    let dev_y = ts.ty + (y - cell_pad) * ts.sy + ascent_dev + mask.top;
                    blend_mask(pm, mask, dev_x, dev_y, rgba);
                }
            }
            _ => {
                if !c.is_whitespace() {
                    draw_tofu(pm, pen, y, px, rgba, ts);
                }
            }
        }
        pen += adv;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_is_monotonic_and_truncate_fits() {
        // holds with a system font and with the tofu fallback alike
        let short = measure("hi", 1.6);
        let long = measure("hello world", 1.6);
        assert!(long > short);
        let cut = truncate_to_width("a somewhat longer line of text", 1.6, 60.0);
        assert!(measure(&cut, 1.6) <= 60.0);
        assert!(cut.ends_with(".."));
        assert_eq!(truncate_to_width("ok", 1.6, 100.0), "ok");
    }

    #[test]
    fn draw_marks_pixels() {
        let mut pm = Pixmap::new(200, 40).unwrap();
        draw(
            &mut pm,
            "Test 한글 123",
            4.0,
            4.0,
            2.0,
            (0, 0, 0, 255),
            Transform::identity(),
        );
        let drawn = pm.data().chunks_exact(4).any(|p| p[3] > 0);
        assert!(drawn, "text must rasterize via system font or tofu boxes");
    }

    #[test]
    fn rotated_transform_draws_tofu_without_panicking() {
        let mut pm = Pixmap::new(60, 60).unwrap();
        draw(
            &mut pm,
            "R",
            10.0,
            10.0,
            2.0,
            (0, 0, 0, 255),
            Transform::from_rotate_at(30.0, 30.0, 30.0),
        );
        let drawn = pm.data().chunks_exact(4).any(|p| p[3] > 0);
        assert!(drawn);
    }

    #[test]
    fn uncovered_characters_still_measure_and_draw() {
        // a Plane-16 private-use char no UI font covers: tofu advance + box
        let w = measure("\u{10FFFD}", 2.0);
        assert_eq!(w, TOFU_ADV * 2.0);
        let mut pm = Pixmap::new(40, 30).unwrap();
        draw(&mut pm, "\u{10FFFD}", 4.0, 4.0, 2.0, (0, 0, 0, 255), Transform::identity());
        assert!(pm.data().chunks_exact(4).any(|p| p[3] > 0));
        // whitespace advances but draws nothing
        let mut pm = Pixmap::new(40, 30).unwrap();
        draw(&mut pm, " ", 4.0, 4.0, 2.0, (0, 0, 0, 255), Transform::identity());
        assert!(pm.data().chunks_exact(4).all(|p| p[3] == 0));
        assert!(measure(" ", 2.0) > 0.0);
    }
}
