//! All drawing: the cat, desk, keyboard, paws, accessories, particles,
//! stats bubble, toasts, the copy-event fish and the clipboard panel.
//! Pure vector art via tiny-skia on a 240x256 logical cat canvas (the panel
//! canvas in `crate::panel` is larger), multiplied by a global scale.

use crate::clipboard::ClipStore;
use crate::i18n::{self, t, Lang, Msg};
use crate::panel as pl;
use crate::panel::Panel;
use crate::sysfont;
use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint,
    Rect, Stroke, Transform,
};

pub const CANVAS_W: f32 = 240.0;
pub const CANVAS_H: f32 = 256.0;

// palette
const OUTLINE: (u8, u8, u8, u8) = (84, 72, 58, 255);
const FUR: (u8, u8, u8, u8) = (250, 247, 242, 255);
const EAR_PINK: (u8, u8, u8, u8) = (247, 200, 207, 255);
const BLUSH: (u8, u8, u8, u8) = (245, 168, 176, 255);
const DESK_TOP: (u8, u8, u8, u8) = (207, 159, 110, 255);
const DESK_FRONT: (u8, u8, u8, u8) = (179, 133, 79, 255);
const KEY_BASE: (u8, u8, u8, u8) = (78, 85, 102, 255);
const KEY_CAP: (u8, u8, u8, u8) = (153, 161, 181, 255);
const TEXT: (u8, u8, u8, u8) = (84, 72, 58, 255);
const TEXT_DIM: (u8, u8, u8, u8) = (149, 138, 124, 255);
const FISH_BLUE: (u8, u8, u8) = (108, 160, 220);
const ROW_SEL: (u8, u8, u8, u8) = (208, 228, 248, 200);
const ROW_HOVER: (u8, u8, u8, u8) = (226, 238, 250, 150);
const PIN_GOLD: (u8, u8, u8, u8) = (242, 201, 76, 255);

#[derive(Clone, Copy, PartialEq)]
pub enum Accessory {
    None,
    Scarf,
    Glasses,
    Beanie,
    Headphones,
    Crown,
    Wizard,
    Sprout,
    Clover,
    Pudding,
}

impl Accessory {
    pub fn from_id(id: usize) -> Accessory {
        match id {
            1 => Accessory::Scarf,
            2 => Accessory::Glasses,
            3 => Accessory::Beanie,
            4 => Accessory::Headphones,
            5 => Accessory::Crown,
            6 => Accessory::Wizard,
            7 => Accessory::Sprout,
            8 => Accessory::Clover,
            9 => Accessory::Pudding,
            _ => Accessory::None,
        }
    }
}

#[derive(Clone, Copy)]
pub enum ParticleKind {
    Heart,
    Star,
    Zzz,
    Sparkle,
}

#[derive(Clone)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32, // 1 -> 0
    pub kind: ParticleKind,
    pub size: f32,
    pub spin: f32,
}

pub struct BubbleData {
    pub level: u32,
    pub pct: f32,
    pub keys: u64,
    pub clicks: u64,
    pub copies: u64,
    pub minutes: u32,
}

// ---- copy-event fish ---------------------------------------------------------

/// Identifies the app a clip was copied from, drawn on the fish the cat eats.
/// `icon` (when the backend could extract one) is a small premultiplied
/// pixmap; otherwise a colored letter badge is used.
pub struct Badge {
    pub letter: char,
    pub color: (u8, u8, u8),
    pub icon: Option<Pixmap>,
}

const BADGE_PALETTE: [(u8, u8, u8); 8] = [
    (108, 160, 220), // blue
    (126, 201, 110), // green
    (240, 148, 86),  // orange
    (199, 128, 232), // purple
    (240, 98, 146),  // pink
    (96, 196, 201),  // teal
    (242, 201, 76),  // gold
    (146, 156, 222), // periwinkle
];

/// Stable per-app color (FNV-1a over the name into the badge palette); shared
/// by the fish badge and the panel rows so an app always shows the same color.
pub fn source_color(name: &str) -> (u8, u8, u8) {
    let mut h: u32 = 0x811c_9dc5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    BADGE_PALETTE[(h as usize) % BADGE_PALETTE.len()]
}

impl Badge {
    pub fn from_source(source: Option<&str>) -> Badge {
        let Some(name) = source.filter(|s| !s.is_empty()) else {
            return Badge {
                letter: '*',
                color: FISH_BLUE,
                icon: None,
            };
        };
        let letter = name
            .chars()
            .find(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or('*');
        Badge {
            letter,
            color: source_color(name),
            icon: None,
        }
    }

    /// Attaches a real app icon from straight-alpha RGBA pixels (size x size).
    pub fn set_icon_rgba(&mut self, size: u32, rgba: &[u8]) {
        if size == 0 || rgba.len() != (size * size * 4) as usize {
            return;
        }
        // premultiply for tiny-skia
        let mut data = rgba.to_vec();
        for px in data.chunks_exact_mut(4) {
            let a = px[3] as u32;
            px[0] = ((px[0] as u32 * a) / 255) as u8;
            px[1] = ((px[1] as u32 * a) / 255) as u8;
            px[2] = ((px[2] as u32 * a) / 255) as u8;
        }
        self.icon = Pixmap::from_vec(
            data,
            tiny_skia::IntSize::from_wh(size, size).unwrap(),
        );
    }
}

/// Where/how to draw the in-flight fish this frame (cat-local coords).
pub struct FishView<'a> {
    pub x: f32,
    pub y: f32,
    pub rot: f32, // degrees
    pub scale: f32,
    pub badge: &'a Badge,
}

pub struct Scene<'a> {
    pub paw_l: f32,
    pub paw_r: f32,
    pub blink: f32,
    pub happy: f32,
    pub sleep: f32,
    pub excite: f32,
    pub squash: f32,
    pub breath: f32,
    pub tail_phase: f32,
    pub mouth_open: f32,
    pub accessory: Accessory,
    pub particles: &'a [Particle],
    pub fish: Option<FishView<'a>>,
    pub bubble: Option<BubbleData>,
    pub bubble_alpha: f32,
    pub toast: Option<(&'a str, f32)>,
    /// First-run guidance banner (the panel hotkey) shown above the cat until
    /// the user opens the panel for the first time; `None` afterwards.
    pub hotkey_hint: Option<&'a str>,
    pub lang: Lang,
    /// Top-left of the cat canvas inside the window canvas (non-zero when
    /// the clipboard panel is open).
    pub origin: (f32, f32),
}

// ---- small helpers --------------------------------------------------------

fn paint(c: (u8, u8, u8, u8)) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, c.3);
    p.anti_alias = true;
    p
}

fn fade(c: (u8, u8, u8, u8), a: f32) -> (u8, u8, u8, u8) {
    (c.0, c.1, c.2, (c.3 as f32 * a.clamp(0.0, 1.0)) as u8)
}

fn oval(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.push_oval(Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0)?);
    pb.finish()
}

fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.5523 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

struct Cv<'a> {
    pm: &'a mut Pixmap,
    ts: Transform, // global scale (and cat origin where applicable)
}

impl<'a> Cv<'a> {
    fn fill(&mut self, p: &Option<Path>, c: (u8, u8, u8, u8)) {
        self.fill_t(p, c, Transform::identity());
    }
    fn fill_t(&mut self, p: &Option<Path>, c: (u8, u8, u8, u8), t: Transform) {
        if let Some(p) = p {
            self.pm
                .fill_path(p, &paint(c), FillRule::Winding, t.post_concat(self.ts), None);
        }
    }
    fn stroke(&mut self, p: &Option<Path>, c: (u8, u8, u8, u8), w: f32) {
        self.stroke_t(p, c, w, Transform::identity());
    }
    /// Fill then stroke the same path — the outlined-shape pattern repeated all
    /// over `draw_fish` / `draw_accessory`. Equivalent to `fill_t` + `stroke_t`.
    fn filled(
        &mut self,
        p: &Option<Path>,
        fill: (u8, u8, u8, u8),
        outline: (u8, u8, u8, u8),
        w: f32,
        t: Transform,
    ) {
        self.fill_t(p, fill, t);
        self.stroke_t(p, outline, w, t);
    }
    fn stroke_t(&mut self, p: &Option<Path>, c: (u8, u8, u8, u8), w: f32, t: Transform) {
        if let Some(p) = p {
            let stroke = Stroke {
                width: w,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Default::default()
            };
            self.pm
                .stroke_path(p, &paint(c), &stroke, t.post_concat(self.ts), None);
        }
    }
    /// Text via the system font (tofu-box fallback; see [`crate::sysfont`]).
    fn ui_text(&mut self, s: &str, x: f32, y: f32, px: f32, c: (u8, u8, u8, u8)) {
        sysfont::draw(self.pm, s, x, y, px, c, self.ts);
    }
    /// Faux-bold UI text: a horizontal double-strike. The glyph advance is
    /// unchanged, so callers can still measure it like normal text. Used to
    /// emphasize the start of a clip row.
    fn ui_text_bold(&mut self, s: &str, x: f32, y: f32, px: f32, c: (u8, u8, u8, u8)) {
        sysfont::draw(self.pm, s, x, y, px, c, self.ts);
        sysfont::draw(self.pm, s, x + px * 0.35, y, px, c, self.ts);
    }
    fn line(&mut self, pts: &[(f32, f32)], c: (u8, u8, u8, u8), w: f32) {
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(*x, *y);
            } else {
                pb.line_to(*x, *y);
            }
        }
        self.stroke(&pb.finish(), c, w);
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ---- main render ----------------------------------------------------------

/// Soft background colour for the portable backend's opaque "card".
pub const CARD_BG: (u8, u8, u8, u8) = (216, 231, 246, 255);

/// Render onto a fully transparent canvas — the native layered-window backend,
/// where transparent pixels let the desktop show through and are click-through.
pub fn render(pm: &mut Pixmap, sc: &Scene, scale: f32) {
    pm.fill(tiny_skia::Color::TRANSPARENT);
    draw_scene(pm, sc, scale);
}

/// Render onto an opaque rounded "card" — used by the portable backend, whose
/// pixel buffer (softbuffer) cannot carry per-pixel alpha to the compositor.
pub fn render_card(pm: &mut Pixmap, sc: &Scene, scale: f32) {
    pm.fill(tiny_skia::Color::from_rgba8(
        CARD_BG.0, CARD_BG.1, CARD_BG.2, CARD_BG.3,
    ));
    let w = pm.width() as f32;
    let h = pm.height() as f32;
    {
        let mut cv = Cv {
            pm,
            ts: Transform::identity(),
        };
        // subtle inset frame so the window reads as a tidy little widget
        let frame = round_rect(3.0, 3.0, w - 6.0, h - 6.0, 14.0);
        cv.stroke(&frame, (255, 255, 255, 150), 3.0);
        cv.stroke(&frame, fade(OUTLINE, 0.30), 1.5);
    }
    draw_scene(pm, sc, scale);
}

fn draw_scene(pm: &mut Pixmap, sc: &Scene, scale: f32) {
    let mut cv = Cv {
        pm,
        // origin is in physical pixels (the cat block's top-left in the union
        // canvas); translate after the scale so only the cat is scaled.
        ts: Transform::from_scale(scale, scale).post_translate(sc.origin.0, sc.origin.1),
    };

    let breath_dy = -sc.breath * 1.6;
    // sleeping head droop
    let tilt = -7.0 * sc.sleep;
    let head_t = Transform::from_rotate_at(tilt, 120.0, 166.0).post_translate(0.0, breath_dy);
    // click squash: compress vertically around desk line
    let squash_t = if sc.squash > 0.0 {
        let s = 1.0 - 0.06 * sc.squash;
        Transform::from_translate(0.0, 212.0)
            .pre_scale(1.0, s)
            .pre_translate(0.0, -212.0)
    } else {
        Transform::identity()
    };
    let body_t = squash_t.pre_translate(0.0, breath_dy * 0.5);
    let head_t = head_t.post_concat(squash_t);

    // ground shadow
    cv.fill(&oval(120.0, 244.0, 102.0, 8.0), (0, 0, 0, 26));

    // tail (behind everything)
    {
        let sway = (sc.tail_phase).sin() * 7.0;
        let sway2 = (sc.tail_phase * 0.7 + 1.2).sin() * 5.0;
        let mut pb = PathBuilder::new();
        pb.move_to(64.0, 196.0);
        pb.quad_to(28.0 + sway2, 188.0, 30.0 + sway, 156.0);
        let p = pb.finish();
        cv.stroke_t(&p, OUTLINE, 14.0, body_t);
        cv.stroke_t(&p, FUR, 9.0, body_t);
    }

    // body
    let body = oval(120.0, 182.0, 64.0, 44.0);
    cv.filled(&body, FUR, OUTLINE, 3.0, body_t);

    // ears (under head outline)
    {
        let mut le = PathBuilder::new();
        le.move_to(72.0, 98.0);
        le.line_to(78.0, 56.0);
        le.line_to(108.0, 76.0);
        le.close();
        let le = le.finish();
        let mut re = PathBuilder::new();
        re.move_to(168.0, 98.0);
        re.line_to(162.0, 56.0);
        re.line_to(132.0, 76.0);
        re.close();
        let re = re.finish();
        cv.filled(&le, FUR, OUTLINE, 3.0, head_t);
        cv.filled(&re, FUR, OUTLINE, 3.0, head_t);
        // inner pink
        let mut li = PathBuilder::new();
        li.move_to(80.0, 90.0);
        li.line_to(83.0, 66.0);
        li.line_to(100.0, 78.0);
        li.close();
        cv.fill_t(&li.finish(), EAR_PINK, head_t);
        let mut ri = PathBuilder::new();
        ri.move_to(160.0, 90.0);
        ri.line_to(157.0, 66.0);
        ri.line_to(140.0, 78.0);
        ri.close();
        cv.fill_t(&ri.finish(), EAR_PINK, head_t);
    }

    // head
    let head = oval(120.0, 128.0, 66.0, 52.0);
    cv.filled(&head, FUR, OUTLINE, 3.0, head_t);

    // face
    draw_face(&mut cv, sc, head_t);

    // accessory (on head, over ears)
    draw_accessory(&mut cv, sc.accessory, head_t);

    // desk
    cv.fill(&round_rect(16.0, 222.0, 208.0, 20.0, 6.0), DESK_FRONT);
    let top = round_rect(12.0, 212.0, 216.0, 14.0, 7.0);
    cv.fill(&top, DESK_TOP);
    cv.stroke(&top, fade(OUTLINE, 0.55), 2.0);

    // keyboard
    {
        let base = round_rect(74.0, 200.0, 92.0, 18.0, 5.0);
        cv.fill(&base, KEY_BASE);
        cv.stroke(&base, fade(OUTLINE, 0.7), 2.0);
        for row in 0..2 {
            let y = 203.5 + row as f32 * 7.0;
            let n = 7 - row; // 7 keys then 6
            let total = n as f32 * 12.0;
            let x0 = 120.0 - total / 2.0 + 1.0;
            for i in 0..n {
                cv.fill(
                    &round_rect(x0 + i as f32 * 12.0, y, 10.0, 5.0, 1.5),
                    KEY_CAP,
                );
            }
        }
    }

    // paws
    draw_paw(&mut cv, 96.0, sc.paw_l, false, breath_dy);
    draw_paw(&mut cv, 144.0, sc.paw_r, true, breath_dy);

    // in-flight fish (over the cat, under particles/toast)
    if let Some(f) = &sc.fish {
        draw_fish(&mut cv, f);
    }

    // particles
    for p in sc.particles {
        draw_particle(&mut cv, p);
    }

    // toast pill
    if let Some((text, a)) = sc.toast {
        if a > 0.01 {
            let w = sysfont::measure(text, 2.0) + 20.0;
            let x = 120.0 - w / 2.0;
            let pill = round_rect(x, 80.0, w, 20.0, 9.0);
            cv.fill(&pill, fade((255, 233, 168, 255), a));
            cv.stroke(&pill, fade(OUTLINE, a), 2.0);
            cv.ui_text(text, x + 10.0, 83.0, 2.0, fade(TEXT, a));
        }
    }

    // stats bubble
    if sc.bubble_alpha > 0.01 {
        if let Some(b) = &sc.bubble {
            draw_bubble(&mut cv, b, sc.bubble_alpha, sc.lang);
        }
    }

    // first-run hotkey hint: a pill in the clear space above the cat's ears.
    // Only shown while the panel is closed, so `cv` here is scale-only (origin
    // is (0,0)) and these are window coordinates.
    if let Some(hint) = sc.hotkey_hint {
        let w = (sysfont::measure(hint, 1.7) + 22.0).min(CANVAS_W - 8.0);
        let x = (CANVAS_W - w) / 2.0;
        let pill = round_rect(x, 6.0, w, 20.0, 9.0);
        cv.fill(&pill, (255, 233, 168, 255));
        cv.stroke(&pill, OUTLINE, 2.0);
        cv.ui_text(hint, x + 11.0, 9.0, 1.7, TEXT);
    }
}

fn draw_face(cv: &mut Cv, sc: &Scene, t: Transform) {
    let closed = sc.blink.max(sc.sleep);
    let happy = sc.happy > 0.35 && sc.sleep < 0.5;
    let chase = sc.mouth_open > 0.05 && sc.sleep < 0.5; // eyeing the fish

    for (ex, dir) in [(92.0f32, -1.0f32), (148.0, 1.0)] {
        let _ = dir;
        if happy && !chase {
            // ∩ shaped happy eyes
            let mut pb = PathBuilder::new();
            pb.move_to(ex - 7.0, 124.0);
            pb.quad_to(ex, 115.0, ex + 7.0, 124.0);
            cv.stroke_t(&pb.finish(), OUTLINE, 3.0, t);
        } else if closed > 0.8 {
            // gently closed
            let mut pb = PathBuilder::new();
            pb.move_to(ex - 6.5, 121.0);
            pb.quad_to(ex, 125.0, ex + 6.5, 121.0);
            cv.stroke_t(&pb.finish(), OUTLINE, 2.6, t);
        } else {
            let ry = 5.2 * (1.0 - closed * 0.85);
            // big sparkly eyes while a fish is incoming
            let r = if chase { 6.2 } else { 5.2 };
            cv.fill_t(&oval(ex, 122.0, r, (ry * r / 5.2).max(0.8)), OUTLINE, t);
            if ry > 2.0 {
                cv.fill_t(&oval(ex - 1.6, 120.2, 1.7, 1.7 * (ry / 5.2)), (255, 255, 255, 230), t);
            }
        }
    }

    if sc.mouth_open > 0.05 {
        // open mouth, ready to nom
        let o = sc.mouth_open.clamp(0.0, 1.0);
        let mouth = oval(120.0, 138.0, 4.5 + 3.5 * o, 3.0 + 5.5 * o);
        cv.filled(&mouth, (164, 88, 92, 255), OUTLINE, 2.2, t);
    } else {
        // ω mouth
        let mut pb = PathBuilder::new();
        pb.move_to(112.0, 136.0);
        pb.quad_to(116.0, 141.0, 120.0, 136.5);
        pb.quad_to(124.0, 141.0, 128.0, 136.0);
        cv.stroke_t(&pb.finish(), OUTLINE, 2.4, t);
    }

    // blush
    let blush_a = 0.45 + sc.happy * 0.5;
    cv.fill_t(&oval(78.0, 134.0, 9.0, 4.5), fade(BLUSH, blush_a), t);
    cv.fill_t(&oval(162.0, 134.0, 9.0, 4.5), fade(BLUSH, blush_a), t);

    // sweat drop when very excited
    if sc.excite > 0.65 && sc.sleep < 0.3 {
        let a = ((sc.excite - 0.65) / 0.35).clamp(0.0, 1.0);
        let mut pb = PathBuilder::new();
        pb.move_to(183.0, 94.0);
        pb.quad_to(190.0, 104.0, 183.0, 108.0);
        pb.quad_to(176.0, 104.0, 183.0, 94.0);
        pb.close();
        cv.fill_t(&pb.finish(), fade((154, 200, 240, 235), a), t);
    }
}

fn draw_paw(cv: &mut Cv, cx: f32, press: f32, right: bool, breath_dy: f32) {
    let y = lerp(184.0, 198.0, press) + breath_dy * 0.3;
    let rot = if right { 6.0 } else { -6.0 };
    let t = Transform::from_rotate_at(rot, cx, y);
    let paw = oval(cx, y, 15.0, 11.0);
    cv.filled(&paw, FUR, OUTLINE, 3.0, t);
    // toe separators, more visible when pressed
    let a = 0.35 + press * 0.45;
    for dx in [-5.0f32, 5.0] {
        let mut pb = PathBuilder::new();
        pb.move_to(cx + dx, y + 3.0);
        pb.line_to(cx + dx, y + 9.0);
        cv.stroke_t(&pb.finish(), fade(OUTLINE, a), 2.0, t);
    }
}

// ---- fish -------------------------------------------------------------------

fn lighten(c: (u8, u8, u8), f: f32) -> (u8, u8, u8, u8) {
    let l = |v: u8| (v as f32 + (255.0 - v as f32) * f) as u8;
    (l(c.0), l(c.1), l(c.2), 255)
}

fn darken(c: (u8, u8, u8), f: f32) -> (u8, u8, u8, u8) {
    let d = |v: u8| (v as f32 * (1.0 - f)) as u8;
    (d(c.0), d(c.1), d(c.2), 255)
}

/// Draws the fish at `f.x/f.y` (cat-local), facing left toward the cat.
fn draw_fish(cv: &mut Cv, f: &FishView) {
    let s_at = Transform::from_translate(f.x, f.y)
        .pre_scale(f.scale, f.scale)
        .pre_translate(-f.x, -f.y);
    let t = Transform::from_rotate_at(f.rot, f.x, f.y).pre_concat(s_at);
    let (x, y) = (f.x, f.y);

    let body_c = lighten(f.badge.color, 0.35);
    let dark_c = darken(f.badge.color, 0.18);

    // tail (two-lobe fin at the right/back)
    {
        let mut pb = PathBuilder::new();
        pb.move_to(x + 12.0, y);
        pb.line_to(x + 24.0, y - 9.0);
        pb.quad_to(x + 20.0, y, x + 24.0, y + 9.0);
        pb.close();
        let tail = pb.finish();
        cv.filled(&tail, dark_c, OUTLINE, 2.4, t);
    }
    // body
    let body = oval(x, y, 16.0, 10.0);
    cv.filled(&body, body_c, OUTLINE, 2.6, t);
    // top fin
    {
        let mut pb = PathBuilder::new();
        pb.move_to(x - 4.0, y - 9.0);
        pb.quad_to(x + 1.0, y - 15.0, x + 7.0, y - 8.5);
        let fin = pb.finish();
        cv.filled(&fin, dark_c, OUTLINE, 2.2, t);
    }
    // eye
    cv.fill_t(&oval(x - 10.0, y - 2.5, 1.9, 1.9), OUTLINE, t);

    // badge: real app icon if present, else letter chip
    if let Some(icon) = &f.badge.icon {
        let size = 13.0;
        let chip = round_rect(x - size / 2.0 - 1.5, y - size / 2.0 - 1.5, size + 3.0, size + 3.0, 4.0);
        cv.filled(&chip, (255, 255, 255, 235), fade(OUTLINE, 0.7), 1.6, t);
        let k = size / icon.width() as f32;
        let it = t
            .post_concat(cv.ts)
            .pre_translate(x - size / 2.0, y - size / 2.0)
            .pre_scale(k, k);
        let pp = PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..Default::default()
        };
        cv.pm.draw_pixmap(0, 0, icon.as_ref(), &pp, it, None);
    } else {
        let chip = oval(x + 1.0, y + 0.5, 7.0, 7.0);
        cv.filled(&chip, (255, 255, 255, 235), fade(OUTLINE, 0.7), 1.6, t);
        // the letter stays upright inside the round chip while the fish
        // rotates (sysfont rasterizes under scale+translate transforms only)
        let px = 1.6 * f.scale;
        let s = cv.ts.sx;
        let label = f.badge.letter.to_string();
        let lw = sysfont::measure(&label, px);
        let mut center = [tiny_skia::Point::from_xy(x + 1.0, y + 0.5)];
        t.post_concat(cv.ts).map_points(&mut center);
        let ts = Transform::from_scale(s, s)
            .post_translate(center[0].x - lw * s / 2.0, center[0].y - 3.5 * px * s);
        sysfont::draw(cv.pm, &label, 0.0, 0.0, px, TEXT, ts);
    }
}

fn draw_accessory(cv: &mut Cv, acc: Accessory, t: Transform) {
    match acc {
        Accessory::None => {}
        Accessory::Scarf => {
            let band = round_rect(76.0, 168.0, 88.0, 16.0, 8.0);
            cv.filled(&band, (217, 79, 79, 255), OUTLINE, 2.5, t);
            let knot = round_rect(140.0, 180.0, 14.0, 22.0, 6.0);
            cv.filled(&knot, (185, 61, 61, 255), OUTLINE, 2.5, t);
        }
        Accessory::Glasses => {
            for ex in [92.0f32, 148.0] {
                let ring = oval(ex, 122.0, 13.0, 13.0);
                cv.filled(&ring, (130, 180, 230, 50), (63, 63, 63, 255), 3.0, t);
            }
            let mut pb = PathBuilder::new();
            pb.move_to(105.0, 120.0);
            pb.quad_to(120.0, 114.0, 135.0, 120.0);
            cv.stroke_t(&pb.finish(), (63, 63, 63, 255), 3.0, t);
        }
        Accessory::Beanie => {
            let mut pb = PathBuilder::new();
            pb.move_to(64.0, 92.0);
            pb.quad_to(120.0, 30.0, 176.0, 92.0);
            pb.close();
            let hat = pb.finish();
            cv.filled(&hat, (91, 141, 217, 255), OUTLINE, 3.0, t);
            let brim = round_rect(62.0, 86.0, 116.0, 13.0, 6.0);
            cv.filled(&brim, (74, 118, 184, 255), OUTLINE, 2.5, t);
            let pom = oval(120.0, 54.0, 9.0, 9.0);
            cv.filled(&pom, (240, 240, 240, 255), OUTLINE, 2.5, t);
        }
        Accessory::Headphones => {
            let mut pb = PathBuilder::new();
            pb.move_to(62.0, 110.0);
            pb.quad_to(120.0, 44.0, 178.0, 110.0);
            cv.stroke_t(&pb.finish(), (61, 61, 61, 255), 7.0, t);
            for ex in [62.0f32, 178.0] {
                let cup = oval(ex, 120.0, 11.0, 15.0);
                cv.filled(&cup, (85, 85, 85, 255), (45, 45, 45, 255), 2.5, t);
                cv.fill_t(&oval(ex, 120.0, 6.0, 9.5), (255, 122, 122, 255), t);
            }
        }
        Accessory::Crown => {
            let mut pb = PathBuilder::new();
            pb.move_to(92.0, 84.0);
            pb.line_to(96.0, 56.0);
            pb.line_to(108.0, 72.0);
            pb.line_to(120.0, 50.0);
            pb.line_to(132.0, 72.0);
            pb.line_to(144.0, 56.0);
            pb.line_to(148.0, 84.0);
            pb.close();
            let crown = pb.finish();
            cv.filled(&crown, (242, 201, 76, 255), (180, 140, 30, 255), 3.0, t);
            cv.fill_t(&oval(120.0, 76.0, 4.0, 4.0), (217, 79, 79, 255), t);
            cv.fill_t(&oval(102.0, 78.0, 3.0, 3.0), (79, 130, 217, 255), t);
            cv.fill_t(&oval(138.0, 78.0, 3.0, 3.0), (79, 130, 217, 255), t);
        }
        Accessory::Wizard => {
            let mut pb = PathBuilder::new();
            pb.move_to(70.0, 88.0);
            pb.line_to(120.0, 14.0);
            pb.line_to(170.0, 88.0);
            pb.close();
            let cone = pb.finish();
            cv.filled(&cone, (107, 91, 217, 255), OUTLINE, 3.0, t);
            let brim = round_rect(56.0, 84.0, 128.0, 12.0, 6.0);
            cv.filled(&brim, (88, 73, 184, 255), OUTLINE, 2.5, t);
            star_at(cv, 120.0, 56.0, 9.0, 0.0, (250, 220, 90, 255), t);
            star_at(cv, 102.0, 74.0, 4.5, 0.6, (250, 220, 90, 220), t);
            star_at(cv, 140.0, 70.0, 4.5, 1.2, (250, 220, 90, 220), t);
        }
        Accessory::Sprout => {
            // a tiny sprout poking from the top of the head (cute meme)
            let green = (126, 201, 110, 255);
            let stem = {
                let mut pb = PathBuilder::new();
                pb.move_to(120.0, 82.0);
                pb.line_to(120.0, 66.0);
                pb.finish()
            };
            cv.stroke_t(&stem, (96, 168, 84, 255), 3.0, t);
            let left = {
                let mut pb = PathBuilder::new();
                pb.move_to(120.0, 73.0);
                pb.quad_to(102.0, 64.0, 109.0, 77.0);
                pb.quad_to(115.0, 75.0, 120.0, 73.0);
                pb.close();
                pb.finish()
            };
            cv.filled(&left, green, (96, 168, 84, 255), 1.8, t);
            let right = {
                let mut pb = PathBuilder::new();
                pb.move_to(120.0, 70.0);
                pb.quad_to(138.0, 58.0, 134.0, 72.0);
                pb.quad_to(127.0, 70.0, 120.0, 70.0);
                pb.close();
                pb.finish()
            };
            cv.filled(&right, green, (96, 168, 84, 255), 1.8, t);
        }
        Accessory::Pudding => {
            // a caramel pudding perched on the head, cherry on top
            let custard = (246, 224, 150, 255);
            let caramel = (190, 130, 70, 255);
            let body = {
                let mut pb = PathBuilder::new();
                pb.move_to(102.0, 58.0);
                pb.line_to(138.0, 58.0);
                pb.line_to(152.0, 92.0);
                pb.quad_to(120.0, 99.0, 88.0, 92.0);
                pb.close();
                pb.finish()
            };
            cv.filled(&body, custard, OUTLINE, 2.6, t);
            let glaze = oval(120.0, 58.0, 20.0, 6.5);
            cv.filled(&glaze, caramel, OUTLINE, 2.0, t);
            for (dx, dy) in [(-12.0f32, 64.0f32), (10.0, 65.0)] {
                let mut pb = PathBuilder::new();
                pb.move_to(120.0 + dx, 58.0);
                pb.quad_to(120.0 + dx + 1.0, dy, 120.0 + dx - 1.0, dy + 3.0);
                cv.stroke_t(&pb.finish(), caramel, 3.0, t);
            }
            cv.filled(&oval(120.0, 49.0, 4.5, 4.5), (216, 72, 80, 255), OUTLINE, 1.6, t);
            let mut stem = PathBuilder::new();
            stem.move_to(122.0, 46.0);
            stem.line_to(126.0, 40.0);
            cv.stroke_t(&stem.finish(), (120, 90, 60, 255), 2.0, t);
        }
        Accessory::Clover => {
            // a lucky four-leaf clover on the crown: one heart-leaf, tip at the
            // center, rotated into four; a curved stem trailing down
            let (cx, cy) = (120.0f32, 62.0f32);
            let green = (118, 192, 96, 255);
            let edge = (86, 150, 68, 255);
            let mut stem = PathBuilder::new();
            stem.move_to(cx + 2.0, cy + 3.0);
            stem.quad_to(cx + 11.0, cy + 16.0, cx + 6.0, cy + 27.0);
            cv.stroke_t(&stem.finish(), edge, 3.0, t);
            let leaf = heart_path(cx, cy - 9.0 * 1.05, 9.0);
            let vein = {
                let mut pb = PathBuilder::new();
                pb.move_to(cx, cy - 1.0);
                pb.line_to(cx, cy - 14.0);
                pb.finish()
            };
            for deg in [45.0f32, 135.0, 225.0, 315.0] {
                let lt = t.pre_concat(Transform::from_rotate_at(deg, cx, cy));
                cv.filled(&leaf, green, edge, 2.0, lt);
                cv.stroke_t(&vein, fade(edge, 0.5), 1.3, lt);
            }
            cv.fill_t(&oval(cx, cy, 2.2, 2.2), edge, t);
        }
    }
}

fn star_path(cx: f32, cy: f32, r: f32, rot: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for i in 0..10 {
        let rr = if i % 2 == 0 { r } else { r * 0.45 };
        let a = rot + i as f32 * std::f32::consts::PI / 5.0 - std::f32::consts::FRAC_PI_2;
        let (x, y) = (cx + rr * a.cos(), cy + rr * a.sin());
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    pb.finish()
}

fn star_at(cv: &mut Cv, cx: f32, cy: f32, r: f32, rot: f32, c: (u8, u8, u8, u8), t: Transform) {
    cv.fill_t(&star_path(cx, cy, r, rot), c, t);
}

fn heart_path(cx: f32, cy: f32, s: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(cx, cy + s * 1.05);
    pb.cubic_to(cx - s * 1.6, cy - s * 0.1, cx - s * 0.85, cy - s * 1.2, cx, cy - s * 0.35);
    pb.cubic_to(cx + s * 0.85, cy - s * 1.2, cx + s * 1.6, cy - s * 0.1, cx, cy + s * 1.05);
    pb.close();
    pb.finish()
}

fn draw_particle(cv: &mut Cv, p: &Particle) {
    let a = p.life.clamp(0.0, 1.0);
    match p.kind {
        ParticleKind::Heart => {
            cv.fill(&heart_path(p.x, p.y, p.size), fade((240, 98, 146, 255), a));
        }
        ParticleKind::Star => {
            let t = Transform::from_rotate_at(p.spin * 57.3, p.x, p.y);
            cv.fill_t(&star_path(p.x, p.y, p.size, 0.0), fade((245, 197, 66, 255), a), t);
        }
        ParticleKind::Sparkle => {
            let mut pb = PathBuilder::new();
            let (s1, s2) = (p.size, p.size * 0.38);
            pb.move_to(p.x, p.y - s1);
            pb.quad_to(p.x + s2 * 0.4, p.y - s2 * 0.4, p.x + s1, p.y);
            pb.quad_to(p.x + s2 * 0.4, p.y + s2 * 0.4, p.x, p.y + s1);
            pb.quad_to(p.x - s2 * 0.4, p.y + s2 * 0.4, p.x - s1, p.y);
            pb.quad_to(p.x - s2 * 0.4, p.y - s2 * 0.4, p.x, p.y - s1);
            pb.close();
            cv.fill(&pb.finish(), fade((255, 245, 200, 255), a));
        }
        ParticleKind::Zzz => {
            let px = 1.6 + (1.0 - a) * 1.4;
            sysfont::draw(
                cv.pm,
                "Z",
                p.x,
                p.y,
                px,
                fade((122, 156, 201, 255), a),
                cv.ts,
            );
        }
    }
}

fn fmt_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn draw_bubble(cv: &mut Cv, b: &BubbleData, alpha: f32, lang: Lang) {
    let a = alpha;
    let rect = round_rect(14.0, 2.0, 212.0, 84.0, 11.0);
    cv.fill(&rect, fade((255, 255, 255, 242), a));
    cv.stroke(&rect, fade(OUTLINE, a), 2.5);
    // tail pointing to the cat
    {
        let mut pb = PathBuilder::new();
        pb.move_to(112.0, 85.0);
        pb.line_to(128.0, 85.0);
        pb.line_to(121.0, 97.0);
        pb.close();
        let tail = pb.finish();
        cv.fill(&tail, fade((255, 255, 255, 242), a));
        cv.stroke(&tail, fade(OUTLINE, a), 2.0);
        // cover the seam
        if let Some(r) = Rect::from_xywh(113.5, 82.5, 13.0, 4.0) {
            cv.pm.fill_rect(
                r,
                &paint(fade((255, 255, 255, 242), a)),
                cv.ts,
                None,
            );
        }
    }

    let tc = fade(TEXT, a);
    // row 1: level + xp bar
    cv.ui_text(&format!("{} {}", t(lang, Msg::BubbleLv), b.level), 24.0, 8.0, 2.0, tc);
    let bar_bg = round_rect(82.0, 8.5, 134.0, 12.0, 6.0);
    cv.fill(&bar_bg, fade((231, 224, 214, 255), a));
    let w = (134.0 * b.pct.clamp(0.0, 1.0)).max(10.0);
    let bar_fg = round_rect(82.0, 8.5, w, 12.0, 6.0);
    cv.fill(&bar_fg, fade((126, 201, 110, 255), a));
    cv.stroke(&bar_bg, fade(OUTLINE, 0.8 * a), 2.0);

    // rows 2-5: today's keys / clicks / copies / active time
    let px = 1.7;
    let rows: [(Msg, String); 4] = [
        (Msg::BubbleKeys, fmt_thousands(b.keys)),
        (Msg::BubbleClicks, fmt_thousands(b.clicks)),
        (Msg::BubbleClips, fmt_thousands(b.copies)),
        (Msg::BubbleActive, {
            let (h, m) = (b.minutes / 60, b.minutes % 60);
            i18n::fmt_active(lang, h, m)
        }),
    ];
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = 27.0 + i as f32 * 14.0;
        cv.ui_text(t(lang, *label), 24.0, y, px, fade(TEXT_DIM, a));
        cv.ui_text(value, 96.0, y, px, tc);
    }
}

// ---- clipboard panel --------------------------------------------------------

/// Everything the panel needs to draw one frame.
pub struct PanelView<'a> {
    pub panel: &'a Panel,
    pub store: &'a ClipStore,
    pub lang: Lang,
    pub capture: bool,
    /// Short hotkey hint shown in the footer (backend-specific).
    pub hint: &'a str,
    pub caret: bool,
}

/// Draws the clipboard panel card (geometry from [`crate::panel::Layout`]).
/// The card always renders at scale 1.0 (panel units == physical pixels), so
/// the layout is used verbatim with no scale transform — the cat's size never
/// affects the panel.
pub fn draw_panel(pm: &mut Pixmap, v: &PanelView) {
    let mut cv = Cv {
        pm,
        ts: Transform::identity(),
    };
    let lang = v.lang;
    let lt = v.panel.active_layout();

    let card = round_rect(lt.card_x, lt.card_y, lt.card_w, lt.card_h, 12.0);
    cv.fill(&card, (255, 255, 255, 247));
    cv.stroke(&card, OUTLINE, 2.5);

    // header: fish mark + title, vertically centered on the button row
    // (the header strip doubles as the card's move-drag handle)
    let hx = lt.card_x + 10.0;
    let hcy = lt.btn_y + pl::BTN / 2.0;
    {
        let mut pb = PathBuilder::new();
        pb.move_to(hx + 12.0, hcy);
        pb.quad_to(hx + 6.0, hcy - 5.5, hx + 0.5, hcy);
        pb.quad_to(hx + 6.0, hcy + 5.5, hx + 12.0, hcy);
        pb.close();
        let fish = pb.finish();
        cv.fill(&fish, (FISH_BLUE.0, FISH_BLUE.1, FISH_BLUE.2, 255));
        cv.stroke(&fish, OUTLINE, 1.8);
        cv.line(
            &[
                (hx + 12.0, hcy),
                (hx + 16.5, hcy - 4.0),
                (hx + 16.5, hcy + 4.0),
                (hx + 12.0, hcy),
            ],
            OUTLINE,
            1.8,
        );
        cv.fill(&oval(hx + 4.5, hcy - 1.2, 1.0, 1.0), OUTLINE);
    }
    cv.ui_text(t(lang, Msg::PanelTitle), hx + 22.0, hcy - 7.0, 2.0, TEXT);
    if !v.capture {
        // capture paused: small red pause bars next to the title
        let tx = hx + 22.0 + sysfont::measure(t(lang, Msg::PanelTitle), 2.0) + 8.0;
        cv.fill(&round_rect(tx, hcy - 6.5, 3.2, 13.0, 1.3), (217, 79, 79, 255));
        cv.fill(&round_rect(tx + 5.4, hcy - 6.5, 3.2, 13.0, 1.3), (217, 79, 79, 255));
    }

    // header buttons
    draw_btn(&mut cv, lt.btn_view_x, lt.btn_y, BtnIcon::View(v.panel.view == 1));
    draw_btn(&mut cv, lt.btn_filter_x, lt.btn_y, BtnIcon::Filter(v.panel.source.is_some()));
    draw_btn(&mut cv, lt.btn_pause_x, lt.btn_y, BtnIcon::Pause(v.capture));
    draw_btn(&mut cv, lt.btn_clear_x, lt.btn_y, BtnIcon::Trash(v.panel.clear_armed));
    draw_btn(&mut cv, lt.btn_lang_x, lt.btn_y, BtnIcon::Lang(lang));
    draw_btn(&mut cv, lt.btn_close_x, lt.btn_y, BtnIcon::Close);

    // search box
    let (sx, sy, sw, sh) = (lt.search_x, lt.search_y, lt.search_w, lt.search_h);
    let sb = round_rect(sx, sy, sw, sh, 9.0);
    cv.fill(&sb, (243, 240, 235, 255));
    cv.stroke(&sb, fade(OUTLINE, 0.5), 1.6);
    // magnifier
    let scy = sy + sh / 2.0;
    cv.stroke(&oval(sx + 10.0, scy - 1.5, 3.6, 3.6), TEXT_DIM, 1.8);
    cv.line(&[(sx + 13.2, scy + 1.5), (sx + 16.2, scy + 4.5)], TEXT_DIM, 1.8);
    let mut qx = sx + 23.0;
    // active source filter: a chip inside the search box; query starts after
    if let Some(src) = &v.panel.source {
        let dot = source_color(src);
        let label = sysfont::truncate_to_width(src, 1.4, 90.0);
        let cw = sysfont::measure(&label, 1.4) + 18.0;
        let chip = round_rect(qx - 2.0, sy + 3.0, cw, sh - 6.0, 7.0);
        cv.fill(&chip, (208, 228, 248, 255));
        cv.stroke(&chip, fade(OUTLINE, 0.45), 1.2);
        cv.fill(&oval(qx + 4.5, scy, 2.6, 2.6), (dot.0, dot.1, dot.2, 255));
        cv.ui_text(&label, qx + 10.0, sy + 5.0, 1.4, TEXT);
        qx += cw + 4.0;
    }
    let qmax = sx + sw - 8.0 - qx;
    let qpx = 1.8;
    let qy = sy + (sh - 7.0 * qpx) / 2.0;
    if v.panel.query.is_empty() {
        cv.ui_text(t(lang, Msg::SearchHint), qx, qy, qpx, fade(TEXT_DIM, 0.8));
        if v.caret {
            cv.fill(&round_rect(qx - 3.0, sy + 3.0, 1.6, sh - 6.0, 0.8), fade(TEXT, 0.7));
        }
    } else {
        // show the tail of long queries
        let mut q: &str = &v.panel.query;
        while sysfont::measure(q, qpx) > qmax - 6.0 {
            let mut it = q.chars();
            it.next();
            q = it.as_str();
        }
        cv.ui_text(q, qx, qy, qpx, TEXT);
        if v.caret {
            let cx = qx + sysfont::measure(q, qpx) + 2.0;
            cv.fill(&round_rect(cx, sy + 3.0, 1.6, sh - 6.0, 0.8), fade(TEXT, 0.7));
        }
    }

    // rows
    let visible = v.panel.visible(v.store);
    let total = visible.len();
    if total == 0 {
        let msg = if v.store.is_empty() {
            t(lang, Msg::PanelEmpty)
        } else {
            t(lang, Msg::PanelNoMatch)
        };
        let w = sysfont::measure(msg, 1.8);
        cv.ui_text(
            msg,
            lt.card_x + (lt.card_w - w) / 2.0,
            lt.rows_y + lt.row_h * lt.rows as f32 / 2.0 - 8.0,
            1.8,
            TEXT_DIM,
        );
    }
    let hover = v.panel.cursor.filter(|(x, _)| {
        (lt.row_x..=lt.row_x + lt.row_w).contains(x)
    });
    let hover_row = hover.and_then(|(_, y)| v.panel.row_at(y, total));
    let now = crate::clipboard::now_ts();
    // two list styles share this loop: the compact "list" (one body line, thin
    // separators) and the roomier "thumbnail" cards (a rounded box per clip
    // with the body wrapped onto two lines). Only a few y-offsets and the
    // background differ; the click zones (pin | body | delete) are identical.
    let thumb = v.panel.view == 1;
    const BODY_PX: f32 = 1.75;
    const BODY_BOLD: usize = 10;
    // a row's pin ★ under the cursor: drawn as a tooltip after the loop (on top)
    let mut pin_tip: Option<(f32, f32, bool)> = None;
    for i in 0..lt.rows {
        let idx = v.panel.scroll + i;
        let Some(clip) = visible.get(idx) else { break };
        let ry = lt.rows_y + i as f32 * lt.row_h;
        let selected = idx == v.panel.sel;
        let hovered = hover_row == Some(idx);
        // a vertical center for the row's left/right gadgets (pin, delete)
        let mid = if thumb { ry + 13.0 } else { ry + lt.row_h / 2.0 };

        if thumb {
            // every clip is its own rounded card; selection/hover tints it
            let bg = if selected {
                ROW_SEL
            } else if hovered {
                ROW_HOVER
            } else {
                (248, 250, 252, 255)
            };
            let card = round_rect(lt.row_x, ry + 1.0, lt.row_w, lt.row_h - 4.0, 8.0);
            cv.fill(&card, bg);
            cv.stroke(&card, fade(OUTLINE, 0.18), 1.0);
        } else if let Some(bg) = selected.then_some(ROW_SEL).or(hovered.then_some(ROW_HOVER)) {
            cv.fill(&round_rect(lt.row_x, ry + 1.0, lt.row_w, lt.row_h - 2.0, 7.0), bg);
        }

        let quick = idx < pl::QUICK_KEYS;
        let right = lt.row_x + lt.row_w;
        let expanded = v.panel.expanded == Some(clip.id);
        // cursor x, but only for the row actually under the pointer
        let hx = hover.filter(|_| hovered).map(|(x, _)| x);

        // body (the whole clip flattened to one line so more of its content
        // shows; the first few characters are bolded for scannability). The
        // thumbnail view wraps it onto a second line for even more. The body
        // shrinks to leave room for the right-side gadget cluster.
        let tx = lt.row_x + pl::BODY_X;
        let cluster = if expanded {
            2.0 * pl::ACT_ZONE
        } else {
            pl::OVF_ZONE + pl::PIN_ZONE + if quick { 16.0 } else { 0.0 }
        };
        let tmax = right - cluster - tx - 6.0;
        let flat = clip.flattened();
        let (line1, line2) = if thumb {
            sysfont::wrap_two(&flat, BODY_PX, tmax)
        } else {
            (sysfont::truncate_to_width(&flat, BODY_PX, tmax), String::new())
        };
        let lead: String = line1.chars().take(BODY_BOLD).collect();
        let rest: String = line1.chars().skip(BODY_BOLD).collect();
        cv.ui_text_bold(&lead, tx, ry + 4.5, BODY_PX, TEXT);
        if !rest.is_empty() {
            cv.ui_text(&rest, tx + sysfont::measure(&lead, BODY_PX), ry + 4.5, BODY_PX, TEXT);
        }
        if !line2.is_empty() {
            cv.ui_text(&line2, tx, ry + 19.5, BODY_PX, TEXT);
        }

        // meta line (source dot + app - age - size), pinned to the row bottom
        let meta_y = if thumb { ry + lt.row_h - 13.0 } else { ry + 20.5 };
        let mut meta = i18n::time_ago(lang, now.saturating_sub(clip.ts));
        if clip.text.len() > 500 {
            meta = format!("{meta} - {:.1}K", clip.text.len() as f32 / 1024.0);
        }
        let mut mx = tx;
        if let Some(src) = &clip.source {
            let dot = source_color(src);
            cv.fill(&oval(mx + 2.6, meta_y + 4.3, 2.6, 2.6), (dot.0, dot.1, dot.2, 255));
            mx += 9.0;
            meta = format!("{src} - {meta}");
        }
        let meta = sysfont::truncate_to_width(&meta, 1.35, tmax - (mx - tx));
        cv.ui_text(&meta, mx, meta_y, 1.35, TEXT_DIM);

        // right-side gadgets: the revealed action buttons when this row is
        // expanded, else the collapsed [ quick badge ] [ pin ] [ ... ] cluster.
        if expanded {
            let del_cx = right - pl::ACT_ZONE / 2.0;
            let paste_cx = right - pl::ACT_ZONE - pl::ACT_ZONE / 2.0;
            let hot = |cx: f32| hx.is_some_and(|x| (x - cx).abs() <= pl::ACT_ZONE / 2.0);
            draw_row_btn(&mut cv, paste_cx, mid, RowBtn::PasteText, hot(paste_cx));
            draw_row_btn(&mut cv, del_cx, mid, RowBtn::Delete, hot(del_cx));
        } else {
            let ovf_cx = right - pl::OVF_ZONE / 2.0;
            let pin_cx = right - pl::OVF_ZONE - pl::PIN_ZONE / 2.0;
            let pin_hot = hx.is_some_and(|x| (x - pin_cx).abs() <= pl::PIN_ZONE / 2.0);
            // pin star (now on the right; brighter outline on hover)
            let star = star_path(pin_cx, mid, 6.5, 0.0);
            if clip.pinned {
                cv.fill(&star, PIN_GOLD);
                cv.stroke(&star, (180, 140, 30, 255), 1.4);
            } else {
                cv.stroke(&star, fade(TEXT_DIM, if pin_hot { 0.95 } else { 0.55 }), 1.4);
            }
            if pin_hot {
                // teach the Ctrl/Cmd+P shortcut where the user is already looking
                pin_tip = Some((pin_cx, mid, clip.pinned));
            }
            // "..." overflow toggle (brighter on hover)
            let ovf_hot = hx.is_some_and(|x| x > right - pl::OVF_ZONE);
            let dotc = fade(TEXT_DIM, if ovf_hot { 1.0 } else { 0.72 });
            for dxo in [-3.2_f32, 0.0, 3.2] {
                cv.fill(&oval(ovf_cx + dxo, mid, 1.15, 1.15), dotc);
            }
            // quick-copy badge: the first ten rows answer to Ctrl+0..9
            if quick {
                let qcx = right - pl::OVF_ZONE - pl::PIN_ZONE - 9.0;
                let chip = round_rect(qcx - 6.5, mid - 6.5, 13.0, 13.0, 4.0);
                cv.fill(&chip, (243, 240, 235, 255));
                cv.stroke(&chip, fade(OUTLINE, 0.35), 1.2);
                let d = char::from(b'0' + idx as u8).to_string();
                let dw = sysfont::measure(&d, 1.3);
                cv.ui_text(&d, qcx - dw / 2.0, mid - 4.0, 1.3, fade(TEXT_DIM, 0.9));
            }
        }

        // thin separator between compact rows (cards have their own gap/border)
        if !thumb && i + 1 < lt.rows {
            cv.line(
                &[(lt.row_x + 4.0, ry + lt.row_h), (lt.row_x + lt.row_w - 4.0, ry + lt.row_h)],
                fade(OUTLINE, 0.12),
                1.0,
            );
        }
    }

    // scrollbar
    if total > lt.rows {
        let track_h = lt.row_h * lt.rows as f32 - 4.0;
        let tx = lt.card_x + lt.card_w - 7.0;
        cv.fill(&round_rect(tx, lt.rows_y + 2.0, 3.5, track_h, 1.7), fade(OUTLINE, 0.12));
        let th = (track_h * lt.rows as f32 / total as f32).max(16.0);
        let ty = lt.rows_y + 2.0
            + (track_h - th) * v.panel.scroll as f32 / (total - lt.rows) as f32;
        cv.fill(&round_rect(tx, ty, 3.5, th, 1.7), fade(OUTLINE, 0.45));
    }

    // footer: count + hotkey hint, then a keyboard-shortcut help line
    cv.line(
        &[(lt.row_x, lt.footer_y - 1.0), (lt.row_x + lt.row_w, lt.footer_y - 1.0)],
        fade(OUTLINE, 0.18),
        1.0,
    );
    let count = i18n::clip_count(lang, v.store.len(), v.store.pinned_count());
    cv.ui_text(&count, lt.card_x + 10.0, lt.footer_y + 3.0, 1.4, TEXT_DIM);
    let hw = sysfont::measure(v.hint, 1.4);
    cv.ui_text(
        v.hint,
        lt.card_x + lt.card_w - 10.0 - hw,
        lt.footer_y + 3.0,
        1.4,
        fade(TEXT_DIM, 0.8),
    );
    let keys = sysfont::truncate_to_width(t(lang, Msg::FooterKeys), 1.25, lt.row_w);
    let kw = sysfont::measure(&keys, 1.25);
    cv.ui_text(
        &keys,
        lt.card_x + (lt.card_w - kw) / 2.0,
        lt.footer_y + 16.0,
        1.25,
        fade(TEXT_DIM, 0.75),
    );

    // resize grip: three diagonal score lines in the bottom-right corner
    {
        let (gx, gy) = (lt.card_x + lt.card_w, lt.card_y + lt.card_h);
        for i in 0..3 {
            let d = 5.0 + i as f32 * 4.0;
            cv.line(&[(gx - d, gy - 3.0), (gx - 3.0, gy - d)], fade(OUTLINE, 0.45), 1.6);
        }
    }

    // header-icon tooltips: drawn last so they sit on top of everything. Shown
    // while the cursor rests on one of the header buttons.
    if let Some((cx, cy)) = v.panel.cursor {
        let on = |bx: f32| {
            cx >= bx - 2.0
                && cx <= bx + pl::BTN + 2.0
                && (lt.btn_y - 2.0..=lt.btn_y + pl::BTN + 2.0).contains(&cy)
        };
        let tip = if on(lt.btn_view_x) {
            Some((lt.btn_view_x, Msg::TipView))
        } else if on(lt.btn_filter_x) {
            Some((lt.btn_filter_x, Msg::TipFilter))
        } else if on(lt.btn_pause_x) {
            Some((lt.btn_pause_x, if v.capture { Msg::TipPause } else { Msg::TipResume }))
        } else if on(lt.btn_clear_x) {
            Some((lt.btn_clear_x, Msg::TipClear))
        } else if on(lt.btn_lang_x) {
            Some((lt.btn_lang_x, Msg::TipLang))
        } else if on(lt.btn_close_x) {
            Some((lt.btn_close_x, Msg::TipClose))
        } else {
            None
        };
        if let Some((bx, msg)) = tip {
            draw_tooltip(&mut cv, &lt, bx + pl::BTN / 2.0, lt.btn_y + pl::BTN + 5.0, t(lang, msg));
        }
    }

    // pin ★ tooltip: teaches the keyboard shortcut at the point of use. The
    // cursor can be over a header button or a row pin, never both, so this and
    // the header tooltip above are mutually exclusive.
    if let Some((cx, cy, pinned)) = pin_tip {
        let msg = if pinned { Msg::TipUnpin } else { Msg::TipPin };
        draw_tooltip(&mut cv, &lt, cx, cy - 22.0, t(lang, msg));
    }
}

/// A small dark tooltip centered on `anchor_x` with its top at `top_y`, clamped
/// inside the card. Used by the header icons and the per-row pin ★.
fn draw_tooltip(cv: &mut Cv, lt: &pl::Layout, anchor_x: f32, top_y: f32, label: &str) {
    let px = 1.4;
    let pad = 6.0;
    let w = sysfont::measure(label, px) + pad * 2.0;
    let h = 15.0;
    let x = (anchor_x - w / 2.0).clamp(lt.card_x + 4.0, lt.card_x + lt.card_w - 4.0 - w);
    let y = top_y.clamp(lt.card_y + 4.0, lt.card_y + lt.card_h - h - 4.0);
    let bg = round_rect(x, y, w, h, 5.0);
    cv.fill(&bg, (54, 47, 38, 240));
    cv.ui_text(label, x + pad, y + (h - 7.0 * px) / 2.0, px, (250, 248, 244, 255));
}

/// A revealed per-row action button.
enum RowBtn {
    PasteText,
    Delete,
}

/// Draws an 18px revealed row-action button (paste-as-text / delete), styled
/// like the header [`draw_btn`] buttons so the panel stays visually consistent.
fn draw_row_btn(cv: &mut Cv, cx: f32, cy: f32, kind: RowBtn, hot: bool) {
    let b = pl::BTN;
    let bg = round_rect(cx - b / 2.0, cy - b / 2.0, b, b, 6.0);
    match kind {
        RowBtn::Delete => {
            if hot {
                cv.fill(&bg, (250, 224, 224, 255));
                cv.stroke(&bg, (217, 79, 79, 255), 1.6);
            } else {
                cv.fill(&bg, (243, 240, 235, 255));
                cv.stroke(&bg, fade(OUTLINE, 0.45), 1.6);
            }
            let c = if hot { (217, 79, 79, 255) } else { TEXT };
            cv.line(&[(cx - 3.4, cy - 3.4), (cx + 3.4, cy + 3.4)], c, 1.9);
            cv.line(&[(cx - 3.4, cy + 3.4), (cx + 3.4, cy - 3.4)], c, 1.9);
        }
        RowBtn::PasteText => {
            if hot {
                cv.fill(&bg, (208, 228, 248, 255));
                cv.stroke(&bg, (91, 141, 217, 255), 1.6);
            } else {
                cv.fill(&bg, (243, 240, 235, 255));
                cv.stroke(&bg, fade(OUTLINE, 0.45), 1.6);
            }
            let c = if hot { (74, 118, 184, 255) } else { TEXT };
            // a clipboard with text lines = "paste as text"
            cv.stroke(&round_rect(cx - 3.6, cy - 4.4, 7.2, 9.4, 1.6), c, 1.4);
            cv.fill(&round_rect(cx - 1.9, cy - 5.8, 3.8, 2.6, 0.8), c);
            cv.line(&[(cx - 2.0, cy - 0.6), (cx + 2.0, cy - 0.6)], c, 1.1);
            cv.line(&[(cx - 2.0, cy + 1.7), (cx + 2.0, cy + 1.7)], c, 1.1);
        }
    }
}

enum BtnIcon {
    Filter(bool), // source filter currently active?
    Pause(bool),  // capture currently on?
    Trash(bool),  // clear armed (next press clears)?
    Lang(Lang),
    View(bool), // thumbnail (card) view currently on?
    Close,
}

fn draw_btn(cv: &mut Cv, bx: f32, by: f32, icon: BtnIcon) {
    let b = pl::BTN;
    let bg = round_rect(bx, by, b, b, 6.0);
    if matches!(icon, BtnIcon::Pause(false) | BtnIcon::Trash(true)) {
        cv.fill(&bg, (250, 224, 224, 255));
        cv.stroke(&bg, (217, 79, 79, 255), 1.6);
    } else if matches!(icon, BtnIcon::Filter(true) | BtnIcon::View(true)) {
        cv.fill(&bg, (208, 228, 248, 255));
        cv.stroke(&bg, (91, 141, 217, 255), 1.6);
    } else {
        cv.fill(&bg, (243, 240, 235, 255));
        cv.stroke(&bg, fade(OUTLINE, 0.45), 1.6);
    }
    let (cx, cy) = (bx + b / 2.0, by + b / 2.0);
    match icon {
        BtnIcon::Filter(active) => {
            // funnel
            let mut pb = PathBuilder::new();
            pb.move_to(cx - 5.2, cy - 4.6);
            pb.line_to(cx + 5.2, cy - 4.6);
            pb.line_to(cx + 1.4, cy + 0.8);
            pb.line_to(cx + 1.4, cy + 5.2);
            pb.line_to(cx - 1.4, cy + 3.8);
            pb.line_to(cx - 1.4, cy + 0.8);
            pb.close();
            let funnel = pb.finish();
            if active {
                cv.fill(&funnel, (74, 118, 184, 255));
            } else {
                cv.stroke(&funnel, TEXT, 1.5);
            }
        }
        BtnIcon::Pause(true) => {
            // capture running -> show pause bars
            cv.fill(&round_rect(cx - 4.0, cy - 4.5, 2.9, 9.0, 1.2), TEXT);
            cv.fill(&round_rect(cx + 1.1, cy - 4.5, 2.9, 9.0, 1.2), TEXT);
        }
        BtnIcon::Pause(false) => {
            // paused -> show play triangle
            let mut pb = PathBuilder::new();
            pb.move_to(cx - 3.1, cy - 4.7);
            pb.line_to(cx + 4.5, cy);
            pb.line_to(cx - 3.1, cy + 4.7);
            pb.close();
            cv.fill(&pb.finish(), (217, 79, 79, 255));
        }
        BtnIcon::Trash(armed) => {
            let c = if armed { (217, 79, 79, 255) } else { TEXT };
            cv.line(&[(cx - 5.0, cy - 4.0), (cx + 5.0, cy - 4.0)], c, 1.7);
            cv.line(&[(cx - 1.8, cy - 5.8), (cx + 1.8, cy - 5.8)], c, 1.7);
            let mut pb = PathBuilder::new();
            pb.move_to(cx - 3.8, cy - 4.0);
            pb.line_to(cx - 3.1, cy + 5.5);
            pb.line_to(cx + 3.1, cy + 5.5);
            pb.line_to(cx + 3.8, cy - 4.0);
            cv.stroke(&pb.finish(), c, 1.7);
        }
        BtnIcon::Lang(lang) => {
            let s = match lang {
                Lang::En => "EN",
                Lang::Ko => "KO",
            };
            let w = sysfont::measure(s, 1.25);
            sysfont::draw(cv.pm, s, cx - w / 2.0, cy - 4.3, 1.25, TEXT, cv.ts);
        }
        BtnIcon::View(active) => {
            // two stacked cards = the list/thumbnail toggle; filled when the
            // roomy card view is on (mirrors the Filter active convention)
            let top = round_rect(cx - 5.0, cy - 4.8, 10.0, 3.6, 1.2);
            let bot = round_rect(cx - 5.0, cy + 1.2, 10.0, 3.6, 1.2);
            if active {
                cv.fill(&top, (74, 118, 184, 255));
                cv.fill(&bot, (74, 118, 184, 255));
            } else {
                cv.stroke(&top, TEXT, 1.5);
                cv.stroke(&bot, TEXT, 1.5);
            }
        }
        BtnIcon::Close => {
            cv.line(&[(cx - 4.0, cy - 4.0), (cx + 4.0, cy + 4.0)], TEXT, 2.0);
            cv.line(&[(cx - 4.0, cy + 4.0), (cx + 4.0, cy - 4.0)], TEXT, 2.0);
        }
    }
}

// ---- tray icon -------------------------------------------------------------

/// Draws a 32x32 cat face (with its little fish) for the tray/window icon.
pub fn draw_icon(pm: &mut Pixmap) {
    draw_icon_scaled(pm, 1.0);
}

/// Same art at `k` times the base 32px size (for .ico generation).
pub fn draw_icon_scaled(pm: &mut Pixmap, k: f32) {
    pm.fill(tiny_skia::Color::TRANSPARENT);
    let mut cv = Cv {
        pm,
        ts: Transform::from_scale(k, k),
    };
    // ears
    let mut le = PathBuilder::new();
    le.move_to(4.0, 14.0);
    le.line_to(5.0, 2.0);
    le.line_to(14.0, 8.0);
    le.close();
    let mut re = PathBuilder::new();
    re.move_to(28.0, 14.0);
    re.line_to(27.0, 2.0);
    re.line_to(18.0, 8.0);
    re.close();
    let (le, re) = (le.finish(), re.finish());
    cv.fill(&le, FUR);
    cv.stroke(&le, OUTLINE, 2.0);
    cv.fill(&re, FUR);
    cv.stroke(&re, OUTLINE, 2.0);
    // head
    let head = oval(16.0, 18.0, 13.5, 12.0);
    cv.fill(&head, FUR);
    cv.stroke(&head, OUTLINE, 2.0);
    // eyes
    cv.fill(&oval(10.5, 17.0, 1.9, 1.9), OUTLINE);
    cv.fill(&oval(21.5, 17.0, 1.9, 1.9), OUTLINE);
    // mouth
    let mut pb = PathBuilder::new();
    pb.move_to(13.0, 22.0);
    pb.quad_to(14.5, 24.0, 16.0, 22.3);
    pb.quad_to(17.5, 24.0, 19.0, 22.0);
    cv.stroke(&pb.finish(), OUTLINE, 1.4);
    // the clipboard fish, held proudly at the bottom-right
    {
        let fc = (FISH_BLUE.0, FISH_BLUE.1, FISH_BLUE.2, 255);
        let body = oval(24.0, 27.5, 6.0, 3.6);
        cv.fill(&body, fc);
        cv.stroke(&body, OUTLINE, 1.6);
        let mut tb = PathBuilder::new();
        tb.move_to(28.5, 27.5);
        tb.line_to(31.5, 24.8);
        tb.line_to(31.5, 30.2);
        tb.close();
        let tail = tb.finish();
        cv.fill(&tail, fc);
        cv.stroke(&tail, OUTLINE, 1.4);
        cv.fill(&oval(21.0, 26.8, 0.9, 0.9), OUTLINE);
    }
}
