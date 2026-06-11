//! Tiny built-in 5x7 pixel font (full printable ASCII) rendered as filled
//! rects with tiny-skia, plus vector Hangul via [`crate::hangul`]. Keeps the
//! binary dependency-free: no font files, no rasterizer crate, and the pixel
//! look matches the pet aesthetic. Unknown characters draw as a hollow box.

use crate::hangul;
use tiny_skia::{Paint, Pixmap, Rect, Transform};

/// Returns the 7 rows (5 LSBs used, MSB-left) for a glyph, or None.
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '"' => [0b01010, 0b01010, 0b01010, 0, 0, 0, 0],
        '#' => [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010],
        '$' => [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100],
        '%' => [0b11000, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b00011],
        '&' => [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101],
        '\'' => [0b00100, 0b00100, 0b01000, 0, 0, 0, 0],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        ',' => [0, 0, 0, 0, 0b00110, 0b00100, 0b01000],
        '-' => [0, 0, 0, 0b01110, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        ';' => [0, 0b01100, 0b01100, 0, 0b01100, 0b00100, 0b01000],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        '@' => [0b01110, 0b10001, 0b00001, 0b01101, 0b10101, 0b10101, 0b01110],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '[' => [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
        '\\' => [0b10000, 0b01000, 0b01000, 0b00100, 0b00010, 0b00010, 0b00001],
        ']' => [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
        '^' => [0b00100, 0b01010, 0b10001, 0, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '`' => [0b01000, 0b00100, 0, 0, 0, 0, 0],
        'a' => [0, 0, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
        'b' => [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
        'c' => [0, 0, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110],
        'd' => [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111],
        'e' => [0, 0, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
        'f' => [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000],
        'g' => [0, 0, 0b01111, 0b10001, 0b01111, 0b00001, 0b01110],
        'h' => [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
        'i' => [0b00100, 0, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        'j' => [0b00010, 0, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100],
        'k' => [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010],
        'l' => [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'm' => [0, 0, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101],
        'n' => [0, 0, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
        'o' => [0, 0, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'p' => [0, 0, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
        'q' => [0, 0, 0b01111, 0b10001, 0b01111, 0b00001, 0b00001],
        'r' => [0, 0, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000],
        's' => [0, 0, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
        't' => [0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110],
        'u' => [0, 0, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101],
        'v' => [0, 0, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'w' => [0, 0, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010],
        'x' => [0, 0, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
        'y' => [0, 0, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'z' => [0, 0, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
        '{' => [0b00110, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00110],
        '|' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        '}' => [0b01100, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01100],
        '~' => [0, 0, 0b01000, 0b10101, 0b00010, 0, 0],
        // '*' renders as a small star, used for unlock toasts
        '*' => [0, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0],
        _ => return None,
    })
}

/// Hollow box for characters we cannot draw (non-Hangul CJK, emoji, ...).
const FALLBACK: [u8; 7] = [
    0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
];

/// Advance per ASCII character cell in font units (5px glyph + 1px gap).
const ADVANCE: f32 = 6.0;
/// Advance per Hangul cell (7px glyph + 1px gap).
const ADVANCE_HANGUL: f32 = hangul::CELL + 1.0;

/// Advance of one character cell in font units (multiply by the pixel size).
/// Also used by [`crate::sysfont`] when it falls back per character.
pub(crate) fn advance(c: char) -> f32 {
    if hangul::is_hangul(c) {
        ADVANCE_HANGUL
    } else {
        ADVANCE
    }
}

/// Pixel width of a string drawn at scale `px`.
pub fn measure(text: &str, px: f32) -> f32 {
    let total: f32 = text.chars().map(advance).sum();
    if total == 0.0 {
        0.0
    } else {
        (total - 1.0) * px
    }
}

/// Truncates `text` so it fits in `max_w` pixels at scale `px`, appending
/// ".." when something was cut.
pub fn truncate_to_width(text: &str, px: f32, max_w: f32) -> String {
    if measure(text, px) <= max_w {
        return text.to_string();
    }
    let ell = 2.0 * ADVANCE * px;
    let mut w = 0.0;
    let mut out = String::new();
    for c in text.chars() {
        let a = advance(c) * px;
        if w + a + ell > max_w {
            out.push_str("..");
            return out;
        }
        out.push(c);
        w += a;
    }
    out
}

/// Draws `text` with its top-left corner at (x, y). ASCII uses the 5x7
/// bitmap (lowercase included); Hangul is composed as vector strokes.
/// `px` is the size of one font pixel; color is straight RGBA.
pub fn draw(
    pm: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    px: f32,
    rgba: (u8, u8, u8, u8),
    ts: Transform,
) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(rgba.0, rgba.1, rgba.2, rgba.3);
    paint.anti_alias = false;

    let mut cx = x;
    for ch in text.chars() {
        if hangul::is_hangul(ch) {
            hangul::draw_glyph(pm, ch, cx, y, px, rgba, ts);
            cx += ADVANCE_HANGUL * px;
            continue;
        }
        let rows = glyph(ch)
            .or_else(|| glyph(ch.to_ascii_uppercase()))
            .unwrap_or(FALLBACK);
        for (ry, row) in rows.iter().enumerate() {
            for bit in 0..5u8 {
                if row & (1 << (4 - bit)) != 0 {
                    if let Some(r) =
                        Rect::from_xywh(cx + bit as f32 * px, y + ry as f32 * px, px, px)
                    {
                        pm.fill_rect(r, &paint, ts, None);
                    }
                }
            }
        }
        cx += ADVANCE * px;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_mixed_script() {
        // 2 ASCII + 1 Hangul = (6 + 6 + 8 - 1) * px
        assert_eq!(measure("ab한", 2.0), (6.0 + 6.0 + 8.0 - 1.0) * 2.0);
        assert_eq!(measure("", 2.0), 0.0);
    }

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate_to_width("hi", 2.0, 100.0), "hi");
        let cut = truncate_to_width("hello world, this is long", 2.0, 60.0);
        assert!(cut.ends_with(".."));
        assert!(measure(&cut, 2.0) <= 60.0);
    }

    #[test]
    fn lowercase_has_glyphs() {
        for c in 'a'..='z' {
            assert!(glyph(c).is_some(), "{c} missing");
        }
        for c in 'A'..='Z' {
            assert!(glyph(c).is_some(), "{c} missing");
        }
    }
}
