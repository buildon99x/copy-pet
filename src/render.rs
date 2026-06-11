//! All drawing: the cat, desk, keyboard, paws, accessories, particles,
//! stats bubble and toasts. Pure vector art via tiny-skia on a 240x256
//! logical canvas, multiplied by a global scale.

use crate::font;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform,
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

#[derive(Clone, Copy, PartialEq)]
pub enum Accessory {
    None,
    Scarf,
    Glasses,
    Beanie,
    Headphones,
    Crown,
    Wizard,
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
    pub minutes: u32,
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
    pub accessory: Accessory,
    pub particles: &'a [Particle],
    pub bubble: Option<BubbleData>,
    pub bubble_alpha: f32,
    pub toast: Option<(&'a str, f32)>,
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
    ts: Transform, // global scale
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
    fn text(&mut self, s: &str, x: f32, y: f32, px: f32, c: (u8, u8, u8, u8)) {
        font::draw(self.pm, s, x, y, px, c, self.ts);
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
        ts: Transform::from_scale(scale, scale),
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
    cv.fill_t(&body, FUR, body_t);
    cv.stroke_t(&body, OUTLINE, 3.0, body_t);

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
        cv.fill_t(&le, FUR, head_t);
        cv.stroke_t(&le, OUTLINE, 3.0, head_t);
        cv.fill_t(&re, FUR, head_t);
        cv.stroke_t(&re, OUTLINE, 3.0, head_t);
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
    cv.fill_t(&head, FUR, head_t);
    cv.stroke_t(&head, OUTLINE, 3.0, head_t);

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

    // particles
    for p in sc.particles {
        draw_particle(&mut cv, p);
    }

    // toast pill
    if let Some((text, a)) = sc.toast {
        if a > 0.01 {
            let w = font::measure(text, 2.0) + 20.0;
            let x = 120.0 - w / 2.0;
            let pill = round_rect(x, 80.0, w, 20.0, 9.0);
            cv.fill(&pill, fade((255, 233, 168, 255), a));
            cv.stroke(&pill, fade(OUTLINE, a), 2.0);
            cv.text(text, x + 10.0, 83.0, 2.0, fade(TEXT, a));
        }
    }

    // stats bubble
    if sc.bubble_alpha > 0.01 {
        if let Some(b) = &sc.bubble {
            draw_bubble(&mut cv, b, sc.bubble_alpha);
        }
    }
}

fn draw_face(cv: &mut Cv, sc: &Scene, t: Transform) {
    let closed = sc.blink.max(sc.sleep);
    let happy = sc.happy > 0.35 && sc.sleep < 0.5;

    for (ex, dir) in [(92.0f32, -1.0f32), (148.0, 1.0)] {
        let _ = dir;
        if happy {
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
            cv.fill_t(&oval(ex, 122.0, 5.2, ry.max(0.8)), OUTLINE, t);
            if ry > 2.0 {
                cv.fill_t(&oval(ex - 1.6, 120.2, 1.7, 1.7 * (ry / 5.2)), (255, 255, 255, 230), t);
            }
        }
    }

    // ω mouth
    {
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
    cv.fill_t(&paw, FUR, t);
    cv.stroke_t(&paw, OUTLINE, 3.0, t);
    // toe separators, more visible when pressed
    let a = 0.35 + press * 0.45;
    for dx in [-5.0f32, 5.0] {
        let mut pb = PathBuilder::new();
        pb.move_to(cx + dx, y + 3.0);
        pb.line_to(cx + dx, y + 9.0);
        cv.stroke_t(&pb.finish(), fade(OUTLINE, a), 2.0, t);
    }
}

fn draw_accessory(cv: &mut Cv, acc: Accessory, t: Transform) {
    match acc {
        Accessory::None => {}
        Accessory::Scarf => {
            let band = round_rect(76.0, 168.0, 88.0, 16.0, 8.0);
            cv.fill_t(&band, (217, 79, 79, 255), t);
            cv.stroke_t(&band, OUTLINE, 2.5, t);
            let knot = round_rect(140.0, 180.0, 14.0, 22.0, 6.0);
            cv.fill_t(&knot, (185, 61, 61, 255), t);
            cv.stroke_t(&knot, OUTLINE, 2.5, t);
        }
        Accessory::Glasses => {
            for ex in [92.0f32, 148.0] {
                let ring = oval(ex, 122.0, 13.0, 13.0);
                cv.fill_t(&ring, (130, 180, 230, 50), t);
                cv.stroke_t(&ring, (63, 63, 63, 255), 3.0, t);
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
            cv.fill_t(&hat, (91, 141, 217, 255), t);
            cv.stroke_t(&hat, OUTLINE, 3.0, t);
            let brim = round_rect(62.0, 86.0, 116.0, 13.0, 6.0);
            cv.fill_t(&brim, (74, 118, 184, 255), t);
            cv.stroke_t(&brim, OUTLINE, 2.5, t);
            let pom = oval(120.0, 38.0, 9.0, 9.0);
            cv.fill_t(&pom, (240, 240, 240, 255), t);
            cv.stroke_t(&pom, OUTLINE, 2.5, t);
        }
        Accessory::Headphones => {
            let mut pb = PathBuilder::new();
            pb.move_to(62.0, 110.0);
            pb.quad_to(120.0, 44.0, 178.0, 110.0);
            cv.stroke_t(&pb.finish(), (61, 61, 61, 255), 7.0, t);
            for ex in [62.0f32, 178.0] {
                let cup = oval(ex, 120.0, 11.0, 15.0);
                cv.fill_t(&cup, (85, 85, 85, 255), t);
                cv.stroke_t(&cup, (45, 45, 45, 255), 2.5, t);
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
            cv.fill_t(&crown, (242, 201, 76, 255), t);
            cv.stroke_t(&crown, (180, 140, 30, 255), 3.0, t);
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
            cv.fill_t(&cone, (107, 91, 217, 255), t);
            cv.stroke_t(&cone, OUTLINE, 3.0, t);
            let brim = round_rect(56.0, 84.0, 128.0, 12.0, 6.0);
            cv.fill_t(&brim, (88, 73, 184, 255), t);
            cv.stroke_t(&brim, OUTLINE, 2.5, t);
            star_at(cv, 120.0, 56.0, 9.0, 0.0, (250, 220, 90, 255), t);
            star_at(cv, 102.0, 74.0, 4.5, 0.6, (250, 220, 90, 220), t);
            star_at(cv, 140.0, 70.0, 4.5, 1.2, (250, 220, 90, 220), t);
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
            font::draw(
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

fn draw_bubble(cv: &mut Cv, b: &BubbleData, alpha: f32) {
    let a = alpha;
    let rect = round_rect(14.0, 4.0, 212.0, 74.0, 11.0);
    cv.fill(&rect, fade((255, 255, 255, 242), a));
    cv.stroke(&rect, fade(OUTLINE, a), 2.5);
    // tail pointing to the cat
    {
        let mut pb = PathBuilder::new();
        pb.move_to(112.0, 77.0);
        pb.line_to(128.0, 77.0);
        pb.line_to(121.0, 90.0);
        pb.close();
        let tail = pb.finish();
        cv.fill(&tail, fade((255, 255, 255, 242), a));
        cv.stroke(&tail, fade(OUTLINE, a), 2.0);
        // cover the seam
        if let Some(r) = Rect::from_xywh(113.5, 74.5, 13.0, 4.0) {
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
    cv.text(&format!("LV {}", b.level), 24.0, 11.0, 2.0, tc);
    let bar_bg = round_rect(82.0, 11.5, 134.0, 12.0, 6.0);
    cv.fill(&bar_bg, fade((231, 224, 214, 255), a));
    let w = (134.0 * b.pct.clamp(0.0, 1.0)).max(10.0);
    let bar_fg = round_rect(82.0, 11.5, w, 12.0, 6.0);
    cv.fill(&bar_fg, fade((126, 201, 110, 255), a));
    cv.stroke(&bar_bg, fade(OUTLINE, 0.8 * a), 2.0);

    // rows 2-4
    cv.text(&format!("KEYS   {}", fmt_thousands(b.keys)), 24.0, 30.0, 2.0, tc);
    cv.text(
        &format!("CLICKS {}", fmt_thousands(b.clicks)),
        24.0,
        46.0,
        2.0,
        tc,
    );
    let (h, m) = (b.minutes / 60, b.minutes % 60);
    cv.text(&format!("ACTIVE {}H {:02}M", h, m), 24.0, 62.0, 2.0, tc);
}

// ---- tray icon -------------------------------------------------------------

/// Draws a 32x32 cat face for the tray/window icon.
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
}
