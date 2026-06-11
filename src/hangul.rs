//! Algorithmic vector Hangul: syllables (U+AC00..U+D7A3) and compatibility
//! jamo are composed at draw time from ~40 jamo stroke shapes and standard
//! initial/vowel/final layout rules, stroked with tiny-skia. No font files —
//! same philosophy as the 5x7 ASCII font in [`crate::font`], which delegates
//! Hangul characters here. Quality target is "legible and cute at >=9 px",
//! not typography; strokes use round caps to match the pet aesthetic.

use tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

/// Hangul glyph cell width/height in font pixels (ASCII glyphs are 5x7; a
/// composed syllable needs a square-ish 7x7 cell to stay legible).
pub const CELL: f32 = 7.0;

pub fn is_hangul(c: char) -> bool {
    matches!(c, '\u{AC00}'..='\u{D7A3}' | '\u{3131}'..='\u{3163}')
}

// ---- jamo base shapes -------------------------------------------------------
// Consonant shapes in a unit box (0..1). Polylines + ovals only.

enum Seg {
    P(&'static [(f32, f32)]),
    O(f32, f32, f32, f32), // cx, cy, rx, ry
}

const G: usize = 0; // ㄱ
const N: usize = 1; // ㄴ
const D: usize = 2; // ㄷ
const R: usize = 3; // ㄹ
const M: usize = 4; // ㅁ
const B: usize = 5; // ㅂ
const S: usize = 6; // ㅅ
const NG: usize = 7; // ㅇ
const J: usize = 8; // ㅈ
const CH: usize = 9; // ㅊ
const K: usize = 10; // ㅋ
const T: usize = 11; // ㅌ
const P: usize = 12; // ㅍ
const H: usize = 13; // ㅎ

static SHAPES: [&[Seg]; 14] = [
    // ㄱ
    &[Seg::P(&[(0.08, 0.12), (0.88, 0.12), (0.88, 0.95)])],
    // ㄴ
    &[Seg::P(&[(0.10, 0.05), (0.10, 0.88), (0.92, 0.88)])],
    // ㄷ
    &[Seg::P(&[(0.90, 0.12), (0.10, 0.12), (0.10, 0.88), (0.90, 0.88)])],
    // ㄹ
    &[Seg::P(&[
        (0.10, 0.10),
        (0.88, 0.10),
        (0.88, 0.48),
        (0.12, 0.48),
        (0.12, 0.88),
        (0.90, 0.88),
    ])],
    // ㅁ
    &[Seg::P(&[
        (0.12, 0.12),
        (0.88, 0.12),
        (0.88, 0.88),
        (0.12, 0.88),
        (0.12, 0.12),
    ])],
    // ㅂ
    &[
        Seg::P(&[(0.12, 0.05), (0.12, 0.88), (0.88, 0.88), (0.88, 0.05)]),
        Seg::P(&[(0.12, 0.45), (0.88, 0.45)]),
    ],
    // ㅅ
    &[
        Seg::P(&[(0.50, 0.08), (0.10, 0.90)]),
        Seg::P(&[(0.50, 0.08), (0.90, 0.90)]),
    ],
    // ㅇ
    &[Seg::O(0.50, 0.52, 0.40, 0.42)],
    // ㅈ
    &[
        Seg::P(&[(0.06, 0.10), (0.94, 0.10)]),
        Seg::P(&[(0.50, 0.10), (0.10, 0.90)]),
        Seg::P(&[(0.50, 0.10), (0.90, 0.90)]),
    ],
    // ㅊ
    &[
        Seg::P(&[(0.36, 0.02), (0.64, 0.02)]),
        Seg::P(&[(0.06, 0.26), (0.94, 0.26)]),
        Seg::P(&[(0.50, 0.26), (0.10, 0.92)]),
        Seg::P(&[(0.50, 0.26), (0.90, 0.92)]),
    ],
    // ㅋ
    &[
        Seg::P(&[(0.08, 0.12), (0.88, 0.12), (0.88, 0.95)]),
        Seg::P(&[(0.08, 0.52), (0.88, 0.52)]),
    ],
    // ㅌ
    &[
        Seg::P(&[(0.88, 0.12), (0.10, 0.12), (0.10, 0.88), (0.90, 0.88)]),
        Seg::P(&[(0.10, 0.50), (0.80, 0.50)]),
    ],
    // ㅍ
    &[
        Seg::P(&[(0.06, 0.12), (0.94, 0.12)]),
        Seg::P(&[(0.32, 0.12), (0.28, 0.88)]),
        Seg::P(&[(0.68, 0.12), (0.72, 0.88)]),
        Seg::P(&[(0.04, 0.88), (0.96, 0.88)]),
    ],
    // ㅎ
    &[
        Seg::P(&[(0.36, 0.0), (0.64, 0.0)]),
        Seg::P(&[(0.12, 0.20), (0.88, 0.20)]),
        Seg::O(0.50, 0.64, 0.32, 0.32),
    ],
];

/// Choseong (initial consonant) index -> (base shape, doubled?).
const INITIALS: [(usize, bool); 19] = [
    (G, false),  // ㄱ
    (G, true),   // ㄲ
    (N, false),  // ㄴ
    (D, false),  // ㄷ
    (D, true),   // ㄸ
    (R, false),  // ㄹ
    (M, false),  // ㅁ
    (B, false),  // ㅂ
    (B, true),   // ㅃ
    (S, false),  // ㅅ
    (S, true),   // ㅆ
    (NG, false), // ㅇ
    (J, false),  // ㅈ
    (J, true),   // ㅉ
    (CH, false), // ㅊ
    (K, false),  // ㅋ
    (T, false),  // ㅌ
    (P, false),  // ㅍ
    (H, false),  // ㅎ
];

/// Jongseong (final consonant) index (1..=27) -> shapes drawn side by side.
static FINALS: [&[usize]; 28] = [
    &[],        // 0: none
    &[G],       // ㄱ
    &[G, G],    // ㄲ
    &[G, S],    // ㄳ
    &[N],       // ㄴ
    &[N, J],    // ㄵ
    &[N, H],    // ㄶ
    &[D],       // ㄷ
    &[R],       // ㄹ
    &[R, G],    // ㄺ
    &[R, M],    // ㄻ
    &[R, B],    // ㄼ
    &[R, S],    // ㄽ
    &[R, T],    // ㄾ
    &[R, P],    // ㄿ
    &[R, H],    // ㅀ
    &[M],       // ㅁ
    &[B],       // ㅂ
    &[B, S],    // ㅄ
    &[S],       // ㅅ
    &[S, S],    // ㅆ
    &[NG],      // ㅇ
    &[J],       // ㅈ
    &[CH],      // ㅊ
    &[K],       // ㅋ
    &[T],       // ㅌ
    &[P],       // ㅍ
    &[H],       // ㅎ
];

/// Compatibility jamo consonants U+3131..=U+314E, same order as Unicode.
static COMPAT_CONSONANTS: [&[usize]; 30] = [
    &[G],
    &[G, G],
    &[G, S],
    &[N],
    &[N, J],
    &[N, H],
    &[D],
    &[D, D],
    &[R],
    &[R, G],
    &[R, M],
    &[R, B],
    &[R, S],
    &[R, T],
    &[R, P],
    &[R, H],
    &[M],
    &[B],
    &[B, B],
    &[B, S],
    &[S],
    &[S, S],
    &[NG],
    &[J],
    &[J, J],
    &[CH],
    &[K],
    &[T],
    &[P],
    &[H],
];

/// Vowel layout class.
#[derive(PartialEq)]
enum VC {
    Vertical,   // ㅏㅐㅑㅒㅓㅔㅕㅖㅣ — vowel right of the initial
    Horizontal, // ㅗㅛㅜㅠㅡ — vowel below the initial
    Mixed,      // ㅘㅙㅚㅝㅞㅟㅢ — both
}

fn vclass(v: usize) -> VC {
    match v {
        0..=7 | 20 => VC::Vertical,
        8 | 12 | 13 | 17 | 18 => VC::Horizontal,
        _ => VC::Mixed,
    }
}

/// (level, jamo decomposition) of a syllable: initial, vowel, final indices.
pub fn decompose(c: char) -> Option<(usize, usize, usize)> {
    let u = c as u32;
    if !(0xAC00..=0xD7A3).contains(&u) {
        return None;
    }
    let s = (u - 0xAC00) as usize;
    Some((s / (21 * 28), (s / 28) % 21, s % 28))
}

// ---- drawing ---------------------------------------------------------------

struct G2<'a> {
    pm: &'a mut Pixmap,
    x: f32,
    y: f32,
    s: f32, // cell size in device px (CELL * px)
    paint: Paint<'static>,
    stroke: Stroke,
    ts: Transform,
}

impl G2<'_> {
    fn line(&mut self, pts: &[(f32, f32)]) {
        let mut pb = PathBuilder::new();
        for (i, (u, v)) in pts.iter().enumerate() {
            let (px, py) = (self.x + u * self.s, self.y + v * self.s);
            if i == 0 {
                pb.move_to(px, py);
            } else {
                pb.line_to(px, py);
            }
        }
        if let Some(p) = pb.finish() {
            self.pm.stroke_path(&p, &self.paint, &self.stroke, self.ts, None);
        }
    }

    fn oval(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        let mut pb = PathBuilder::new();
        let r = Rect::from_xywh(
            self.x + (cx - rx) * self.s,
            self.y + (cy - ry) * self.s,
            rx * 2.0 * self.s,
            ry * 2.0 * self.s,
        );
        if let Some(r) = r {
            pb.push_oval(r);
            if let Some(p) = pb.finish() {
                self.pm.stroke_path(&p, &self.paint, &self.stroke, self.ts, None);
            }
        }
    }

    /// Draws shape `id` mapped into the cell-space box (x0,y0)-(x1,y1).
    fn shape(&mut self, id: usize, bx: (f32, f32, f32, f32)) {
        let (x0, y0, x1, y1) = bx;
        let (w, h) = (x1 - x0, y1 - y0);
        for seg in SHAPES[id] {
            match seg {
                Seg::P(pts) => {
                    let mapped: Vec<(f32, f32)> =
                        pts.iter().map(|(u, v)| (x0 + u * w, y0 + v * h)).collect();
                    self.line(&mapped);
                }
                Seg::O(cx, cy, rx, ry) => {
                    self.oval(x0 + cx * w, y0 + cy * h, rx * w, ry * h)
                }
            }
        }
    }

    /// One or two consonant shapes in a box (two = double/cluster, split).
    fn shapes(&mut self, ids: &[usize], bx: (f32, f32, f32, f32)) {
        let (x0, y0, x1, y1) = bx;
        match ids {
            [a] => self.shape(*a, bx),
            [a, b] => {
                let mid = x0 + (x1 - x0) * 0.5;
                self.shape(*a, (x0, y0, mid - 0.015, y1));
                self.shape(*b, (mid + 0.015, y0, x1, y1));
            }
            _ => {}
        }
    }

    /// Vertical vowel component: bars at `bars` x positions spanning y0..y1,
    /// ticks (short horizontals) at fractions of the bar going from tx0->tx1.
    fn vvowel(&mut self, bars: &[f32], ticks: &[f32], tx0: f32, tx1: f32, y0: f32, y1: f32) {
        for &bx in bars {
            self.line(&[(bx, y0), (bx, y1)]);
        }
        for &f in ticks {
            let ty = y0 + (y1 - y0) * f;
            self.line(&[(tx0, ty), (tx1, ty)]);
        }
    }

    /// Horizontal vowel component: bar at `by` spanning x0..x1 with vertical
    /// ticks at `xs` of length `len` (negative = upward).
    fn hvowel(&mut self, by: f32, x0: f32, x1: f32, xs: &[f32], len: f32) {
        self.line(&[(x0, by), (x1, by)]);
        for &tx in xs {
            self.line(&[(tx, by), (tx, by + len)]);
        }
    }
}

/// Draws the vertical part of a vowel. `hf` squeezes it above a final.
fn draw_vertical_part(g: &mut G2, v: usize, hf: bool) {
    let (y0, y1) = if hf { (0.02, 0.58) } else { (0.03, 0.96) };
    match v {
        0 => g.vvowel(&[0.74], &[0.5], 0.74, 0.95, y0, y1), // ㅏ
        1 => g.vvowel(&[0.70, 0.93], &[0.5], 0.70, 0.93, y0, y1), // ㅐ
        2 => g.vvowel(&[0.74], &[0.30, 0.62], 0.74, 0.95, y0, y1), // ㅑ
        3 => g.vvowel(&[0.70, 0.93], &[0.30, 0.62], 0.70, 0.93, y0, y1), // ㅒ
        4 => g.vvowel(&[0.82], &[0.5], 0.60, 0.82, y0, y1), // ㅓ
        5 => g.vvowel(&[0.74, 0.94], &[0.5], 0.56, 0.74, y0, y1), // ㅔ
        6 => g.vvowel(&[0.82], &[0.30, 0.62], 0.62, 0.82, y0, y1), // ㅕ
        7 => g.vvowel(&[0.74, 0.94], &[0.30, 0.62], 0.58, 0.74, y0, y1), // ㅖ
        20 => g.vvowel(&[0.80], &[], 0.0, 0.0, y0, y1), // ㅣ
        // vertical halves of mixed vowels
        9 => g.vvowel(&[0.80], &[0.5], 0.80, 0.97, 0.02, if hf { 0.58 } else { 0.96 }), // ㅘ
        10 => g.vvowel(&[0.76, 0.95], &[0.5], 0.76, 0.95, 0.02, if hf { 0.58 } else { 0.96 }), // ㅙ
        11 | 16 => g.vvowel(&[0.84], &[], 0.0, 0.0, 0.02, if hf { 0.58 } else { 0.96 }), // ㅚㅟ
        14 => g.vvowel(&[0.84], &[0.5], 0.66, 0.84, 0.02, if hf { 0.58 } else { 0.96 }), // ㅝ
        15 => g.vvowel(&[0.78, 0.96], &[0.5], 0.62, 0.78, 0.02, if hf { 0.58 } else { 0.96 }), // ㅞ
        19 => g.vvowel(&[0.86], &[], 0.0, 0.0, 0.02, if hf { 0.58 } else { 0.96 }), // ㅢ
        _ => {}
    }
}

/// Draws a full syllable cell with top-left at (x, y); the cell is
/// `CELL * px` square. Returns false if `c` is not drawable here.
pub fn draw_glyph(
    pm: &mut Pixmap,
    c: char,
    x: f32,
    y: f32,
    px: f32,
    rgba: (u8, u8, u8, u8),
    ts: Transform,
) -> bool {
    let mut paint = Paint::default();
    paint.set_color_rgba8(rgba.0, rgba.1, rgba.2, rgba.3);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: (px * 0.95).max(0.7),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let mut g = G2 {
        pm,
        x,
        y,
        s: CELL * px,
        paint,
        stroke,
        ts,
    };

    if let Some((ini, v, fin)) = decompose(c) {
        let (shape, dbl) = INITIALS[ini];
        let ini_ids: &[usize] = if dbl { &[shape, shape] } else { &[shape] };
        let fin_ids = FINALS[fin];
        let hf = !fin_ids.is_empty();

        match vclass(v) {
            VC::Vertical => {
                let by1 = if hf { 0.55 } else { 0.88 };
                let by0 = if hf { 0.03 } else { 0.07 };
                g.shapes(ini_ids, (0.03, by0, 0.55, by1));
                draw_vertical_part(&mut g, v, hf);
            }
            VC::Horizontal => {
                let by1 = if hf { 0.36 } else { 0.48 };
                let by0 = if hf { 0.02 } else { 0.08 };
                g.shapes(ini_ids, (0.16, by0, 0.84, by1));
                let bar_y = if hf { 0.50 } else { 0.66 };
                let len = if hf { 0.12 } else { 0.17 };
                match v {
                    8 => g.hvowel(bar_y, 0.04, 0.96, &[0.50], -len), // ㅗ
                    12 => g.hvowel(bar_y, 0.04, 0.96, &[0.36, 0.64], -len), // ㅛ
                    13 => g.hvowel(bar_y, 0.04, 0.96, &[0.50], len), // ㅜ
                    17 => g.hvowel(bar_y, 0.04, 0.96, &[0.36, 0.64], len), // ㅠ
                    18 => g.hvowel(bar_y, 0.04, 0.96, &[], 0.0), // ㅡ
                    _ => {}
                }
            }
            VC::Mixed => {
                let by1 = if hf { 0.38 } else { 0.48 };
                g.shapes(ini_ids, (0.03, if hf { 0.02 } else { 0.05 }, 0.48, by1));
                let bar_y = if hf { 0.48 } else { 0.58 };
                let len = if hf { 0.11 } else { 0.14 };
                match v {
                    9..=11 => g.hvowel(bar_y, 0.03, 0.64, &[0.32], -len), // ㅗ-
                    14..=16 => g.hvowel(bar_y, 0.03, 0.64, &[0.32], len), // ㅜ-
                    19 => g.hvowel(bar_y, 0.03, 0.70, &[], 0.0), // ㅡ-
                    _ => {}
                }
                draw_vertical_part(&mut g, v, hf);
            }
        }

        if hf {
            g.shapes(fin_ids, (0.14, 0.66, 0.86, 0.98));
        }
        return true;
    }

    // standalone compatibility jamo
    let u = c as u32;
    if (0x3131..=0x314E).contains(&u) {
        let ids = COMPAT_CONSONANTS[(u - 0x3131) as usize];
        g.shapes(ids, (0.12, 0.10, 0.88, 0.90));
        return true;
    }
    if (0x314F..=0x3163).contains(&u) {
        let v = (u - 0x314F) as usize;
        match vclass(v) {
            VC::Vertical => draw_vertical_part(&mut g, v, false),
            VC::Horizontal => {
                let bar_y = 0.55;
                match v {
                    8 => g.hvowel(bar_y, 0.06, 0.94, &[0.50], -0.20),
                    12 => g.hvowel(bar_y, 0.06, 0.94, &[0.36, 0.64], -0.20),
                    13 => g.hvowel(bar_y, 0.06, 0.94, &[0.50], 0.20),
                    17 => g.hvowel(bar_y, 0.06, 0.94, &[0.36, 0.64], 0.20),
                    18 => g.hvowel(bar_y, 0.06, 0.94, &[], 0.0),
                    _ => {}
                }
            }
            VC::Mixed => {
                match v {
                    9..=11 => g.hvowel(0.58, 0.03, 0.64, &[0.32], -0.14),
                    14..=16 => g.hvowel(0.58, 0.03, 0.64, &[0.32], 0.14),
                    19 => g.hvowel(0.58, 0.03, 0.70, &[], 0.0),
                    _ => {}
                }
                draw_vertical_part(&mut g, v, false);
            }
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompose_known_syllables() {
        // 한 = ㅎ(18) + ㅏ(0) + ㄴ(4)
        assert_eq!(decompose('한'), Some((18, 0, 4)));
        // 가 = ㄱ(0) + ㅏ(0) + none
        assert_eq!(decompose('가'), Some((0, 0, 0)));
        // 뷁 = ㅂ(7) + ㅞ(15) + ㄺ(9)
        assert_eq!(decompose('뷁'), Some((7, 15, 9)));
        assert_eq!(decompose('A'), None);
    }

    #[test]
    fn every_syllable_renders_pixels() {
        // A spread of syllables covering all layout classes and finals.
        for c in ['가', '힣', '한', '글', '쥐', '의', '뷁', '꽉', '쌍', 'ㅋ', 'ㅔ'] {
            let mut pm = Pixmap::new(32, 32).unwrap();
            assert!(draw_glyph(
                &mut pm,
                c,
                2.0,
                2.0,
                4.0,
                (0, 0, 0, 255),
                Transform::identity()
            ));
            let drawn = pm.data().chunks_exact(4).any(|p| p[3] > 0);
            assert!(drawn, "{c} drew no pixels");
        }
    }

    #[test]
    fn all_initials_vowels_finals_have_shapes() {
        for (i, _) in INITIALS.iter().enumerate() {
            assert!(INITIALS[i].0 < SHAPES.len());
        }
        for f in FINALS.iter().skip(1) {
            assert!(!f.is_empty());
            for id in *f {
                assert!(*id < SHAPES.len());
            }
        }
    }
}
