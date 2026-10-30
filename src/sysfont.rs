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

use ab_glyph::{Font, FontArc, FontRef, FontVec, ScaleFont};
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
/// Collections are inspected for an upright Regular face; collection order
/// is not a style guarantee (face zero can be Thin or Italic).
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

// Only the small SFNT metadata needed for style selection is read here.
// ab_glyph still validates the selected face and parses/rasterizes all glyphs.
#[derive(Debug, Clone)]
struct FaceInfo {
    index: u32,
    weight: u16,
    italic: bool,
    name: String,
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        bytes.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn face_offset(bytes: &[u8], index: u32) -> Option<usize> {
    if bytes.get(..4)? == b"ttcf" {
        if index >= read_u32(bytes, 8)? {
            return None;
        }
        Some(read_u32(
            bytes,
            12usize.checked_add((index as usize).checked_mul(4)?)?,
        )? as usize)
    } else {
        (index == 0).then_some(0)
    }
}

fn table<'a>(bytes: &'a [u8], face: usize, tag: &[u8; 4]) -> Option<&'a [u8]> {
    let count = read_u16(bytes, face.checked_add(4)?)? as usize;
    for i in 0..count {
        let record = face.checked_add(12)?.checked_add(i.checked_mul(16)?)?;
        if bytes.get(record..record.checked_add(4)?)? == tag {
            let start = read_u32(bytes, record.checked_add(8)?)? as usize;
            let len = read_u32(bytes, record.checked_add(12)?)? as usize;
            return bytes.get(start..start.checked_add(len)?);
        }
    }
    None
}

fn face_name(bytes: &[u8]) -> Option<String> {
    let count = read_u16(bytes, 2)? as usize;
    let storage = read_u16(bytes, 4)? as usize;
    for i in 0..count {
        let at = 6 + i * 12;
        // Prefer a Unicode/Windows full font name (includes the style).
        if !matches!(read_u16(bytes, at)?, 0 | 3) || read_u16(bytes, at + 6)? != 4 {
            continue;
        }
        let len = read_u16(bytes, at + 8)? as usize;
        let start = storage.checked_add(read_u16(bytes, at + 10)? as usize)?;
        let raw = bytes.get(start..start.checked_add(len)?)?;
        let utf16: Vec<_> = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        if let Ok(name) = String::from_utf16(&utf16) {
            return Some(name);
        }
    }
    None
}

fn face_info(bytes: &[u8], index: u32) -> Option<FaceInfo> {
    let offset = face_offset(bytes, index)?;
    let os2 = table(bytes, offset, b"OS/2");
    let style = table(bytes, offset, b"head")
        .and_then(|t| read_u16(t, 44))
        .unwrap_or(0);
    Some(FaceInfo {
        index,
        weight: os2
            .and_then(|t| read_u16(t, 4))
            .unwrap_or(if style & 1 != 0 { 700 } else { 400 }),
        italic: style & 2 != 0 || os2.and_then(|t| read_u16(t, 62)).unwrap_or(0) & 1 != 0,
        name: table(bytes, offset, b"name")
            .and_then(face_name)
            .unwrap_or_default(),
    })
}

fn face_count(bytes: &[u8]) -> u32 {
    if bytes.get(..4) == Some(b"ttcf") {
        // The offset array must fit in the input; bound iteration on corrupt files.
        read_u32(bytes, 8)
            .unwrap_or(0)
            .min(bytes.len().saturating_sub(12) as u32 / 4)
    } else {
        1
    }
}

fn select_face(bytes: &[u8], required: &str) -> Option<FaceInfo> {
    (0..face_count(bytes))
        .filter_map(|index| face_info(bytes, index))
        .filter(|face| !face.italic && (350..=500).contains(&face.weight))
        .filter(|face| {
            FontRef::try_from_slice_and_index(bytes, face.index)
                .map(|f| required.chars().all(|c| f.glyph_id(c).0 != 0))
                .unwrap_or(false)
        })
        .min_by_key(|face| (face.weight.abs_diff(400), face.index))
}

fn load_font(path: &str, required: &str) -> Option<FontArc> {
    let bytes = std::fs::read(PathBuf::from(path)).ok()?;
    let face = select_face(&bytes, required)?;
    FontVec::try_from_vec_and_index(bytes, face.index)
        .ok()
        .map(FontArc::new)
}

/// The loaded system fonts: `[0]` primary UI font, `[1..]` fallbacks
/// (Hangul). Empty when nothing could be found — every character then
/// draws as a tofu box.
fn fonts() -> &'static [FontArc] {
    static FONTS: OnceLock<Vec<FontArc>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut out = Vec::new();
        for (slot, required) in candidates().into_iter().zip(["Aa09", "가한글"]) {
            if let Some(font) = slot.iter().find_map(|p| load_font(p, required)) {
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

/// Headless font inventory for platform CI and the `font_diagnostics` example.
/// Includes every candidate face so collection ordering and rejected styles
/// remain visible even when another candidate is selected.
pub fn diagnostics() -> String {
    use std::fmt::Write;
    let mut report = String::new();
    for (slot, required) in candidates().into_iter().zip(["Aa09", "가한글"]) {
        let mut selected = false;
        for path in slot {
            let Ok(bytes) = std::fs::read(&path) else {
                let _ = writeln!(report, "missing {path}");
                continue;
            };
            let choice = select_face(&bytes, required);
            for index in 0..face_count(&bytes) {
                let Some(info) = face_info(&bytes, index) else {
                    continue;
                };
                let Ok(font) = FontRef::try_from_slice_and_index(&bytes, index) else {
                    continue;
                };
                let chosen = !selected && choice.as_ref().map(|f| f.index) == Some(index);
                let scaled = font.as_scaled(1.4 * EM_PER_PX);
                let ascii = (' '..='~').all(|c| font.glyph_id(c).0 != 0);
                let hangul = "가나다라마바사안녕하세요클립캣한글힣"
                    .chars()
                    .all(|c| font.glyph_id(c).0 != 0);
                let _ = writeln!(report,
                    "{} path={path} face={index} name={:?} weight={} italic={} ascii={ascii} hangul={hangul} body_height={:.2} ascent={:.2} descent={:.2}",
                    if chosen { "SELECTED" } else { "candidate" }, info.name, info.weight, info.italic,
                    scaled.height(), scaled.ascent(), scaled.descent());
            }
            selected |= choice.is_some();
        }
    }
    let _ = writeln!(
        report,
        "combined_ascii={} combined_hangul={}",
        (' '..='~').all(|c| glyph_for(c).is_some()),
        "가나다라마바사안녕하세요클립캣한글힣"
            .chars()
            .all(|c| glyph_for(c).is_some())
    );
    report
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
    // Held for the whole string instead of re-locked per glyph: a panel frame
    // draws dozens of strings, each several characters, all on the render
    // thread, so one lock/unlock pair per `draw()` call is enough.
    let mut cache = glyph_cache().lock().unwrap();
    // Shared baseline: derived once from the primary font's ascent and reused
    // for every glyph, so fallback glyphs (e.g. Hangul from the CJK font, whose
    // ascent differs) sit on the same baseline as adjacent Latin text instead
    // of each font's own ascent. Loop-invariant (depends only on `em_dev`), so
    // hoisted out of the per-glyph hot path. `first()` (not `[0]`) because
    // `fonts()` is empty on systems with no usable font — every glyph then takes
    // the `None => draw_tofu` arm and this value is never read, so 0.0 is fine.
    let ascent_dev = fonts()
        .first()
        .map(|f| f.as_scaled(em_dev).ascent())
        .unwrap_or(0.0);
    for c in text.chars() {
        // Resolve the glyph once and derive its advance from the same lookup,
        // rather than calling `advance_of` (which re-runs `glyph_for`).
        let glyph = glyph_for(c);
        let adv = match glyph {
            Some((i, id)) => fonts()[i].as_scaled(px * EM_PER_PX).h_advance(id),
            None => TOFU_ADV * px,
        };
        match glyph {
            Some((fi, id)) if !rotated => {
                let key: GlyphKey = (fi, id.0, (em_dev * 4.0).round() as u32);
                let mask = cache
                    .entry(key)
                    .or_insert_with(|| rasterize(fi, id, em_dev));
                if let Some(mask) = mask {
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

    // Build a collection around real system font tables, with Thin first and
    // Regular second. No font assets are bundled; the outlines are parsed and
    // rasterized by ab_glyph just as they are in production.
    fn thin_first_collection() -> Option<Vec<u8>> {
        let font = fonts().first()?;
        let source = font.font_data();
        let face = select_face(source, "Aa09")?;
        let offset = face_offset(source, face.index)?;
        let tables = read_u16(source, offset + 4)? as usize;
        let directory = source.get(offset..offset + 12 + tables * 16)?;
        let mut bytes = b"ttcf\0\x01\0\0\0\0\0\x02\0\0\0\0\0\0\0\0".to_vec();
        let source_start = bytes.len();
        bytes.extend_from_slice(source);
        for (index, weight) in [100u16, 400].into_iter().enumerate() {
            let start = bytes.len();
            bytes[12 + index * 4..16 + index * 4].copy_from_slice(&(start as u32).to_be_bytes());
            bytes.extend_from_slice(directory);
            for i in 0..tables {
                let record = start + 12 + i * 16;
                let old_offset = read_u32(&bytes, record + 8)? as usize;
                let mut new_offset = source_start + old_offset;
                if &bytes[record..record + 4] == b"OS/2" {
                    let table = table(source, offset, b"OS/2")?;
                    new_offset = bytes.len();
                    bytes.extend_from_slice(table);
                    bytes[new_offset + 4..new_offset + 6].copy_from_slice(&weight.to_be_bytes());
                }
                bytes[record + 8..record + 12].copy_from_slice(&(new_offset as u32).to_be_bytes());
            }
        }
        Some(bytes)
    }

    #[test]
    fn collection_selects_regular_instead_of_thin_first_face() {
        let Some(bytes) = thin_first_collection() else {
            return;
        };
        assert_eq!(face_info(&bytes, 0).unwrap().weight, 100);
        let selected = select_face(&bytes, "Aa09").unwrap();
        assert_eq!(selected.index, 1);
        assert_eq!(selected.weight, 400);
        let font = FontVec::try_from_vec_and_index(bytes, selected.index).unwrap();
        let glyph = font.glyph_id('A').with_scale(13.0);
        assert!(
            font.outline_glyph(glyph).is_some(),
            "selected real font must rasterize"
        );
    }

    #[test]
    fn malformed_font_metadata_is_rejected_without_panicking() {
        for bytes in [
            &b""[..],
            &b"ttcf"[..],
            &b"ttcf\0\x01\0\0\xff\xff\xff\xff"[..],
        ] {
            assert!(select_face(bytes, "A").is_none());
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_system_fonts_cover_english_and_korean_with_regular_faces() {
        let details = diagnostics();
        assert!(
            (' '..='~').all(|c| glyph_for(c).is_some()),
            "ASCII coverage missing:\n{details}"
        );
        assert!(
            "가나다라마바사안녕하세요클립캣한글힣"
                .chars()
                .all(|c| glyph_for(c).is_some()),
            "Hangul coverage missing:\n{details}"
        );
        assert_eq!(fonts().len(), 2, "Both font slots must load:\n{details}");
    }

    #[test]
    fn mixed_body_text_fits_clipboard_row_at_one_and_two_times_scale() {
        for scale in [1.0, 2.0] {
            let mut pm = Pixmap::new((360.0 * scale) as u32, (34.0 * scale) as u32).unwrap();
            draw(
                &mut pm,
                "Clipboard 클립보드 한글 0123456789",
                8.0,
                8.0,
                1.4,
                (30, 35, 40, 255),
                Transform::from_scale(scale, scale),
            );
            let rows: Vec<_> = pm
                .data()
                .chunks_exact(pm.width() as usize * 4)
                .enumerate()
                .filter(|(_, row)| row.chunks(4).any(|p| p[3] != 0))
                .map(|(y, _)| y)
                .collect();
            assert!(!rows.is_empty());
            assert!(
                *rows.first().unwrap() > 0 && *rows.last().unwrap() < pm.height() as usize - 1,
                "mixed glyphs touch row boundary at scale {scale}: {rows:?}\n{}",
                diagnostics()
            );
        }
    }

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
