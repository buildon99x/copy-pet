//! All drawing: the cat, desk, keyboard, paws, accessories, particles,
//! stats bubble, toasts, the copy-event fish and the clipboard panel.
//! Pure vector art via tiny-skia on a 240x256 logical cat canvas (the panel
//! canvas in `crate::panel` is larger), multiplied by a global scale.

use crate::clipboard::ClipStore;
use crate::i18n::{self, t, Lang, Msg};
use crate::panel as pl;
use crate::panel::Panel;
use crate::sysfont;
use crate::tokens;
use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint,
    Rect, Stroke, Transform,
};

pub const CANVAS_W: f32 = 240.0;
pub const CANVAS_H: f32 = 256.0;

// palette — mascot colors from the design tokens (cat.* in colors.json); the
// desk/keyboard are the dark-premium surfaces so the warm cat reads against a
// cool slate slab, matching the reference art.
const OUTLINE: (u8, u8, u8, u8) = (90, 62, 48, 255); // tokens cat.line #5A3E30
const FUR: (u8, u8, u8, u8) = (255, 243, 226, 255); // tokens cat.fur #FFF3E2
const EAR_PINK: (u8, u8, u8, u8) = (255, 213, 207, 255); // tokens cat.innerEar #FFD5CF
const BLUSH: (u8, u8, u8, u8) = (245, 168, 176, 255);
const DESK_TOP: (u8, u8, u8, u8) = (38, 44, 58, 255); // dark shelf top
const DESK_FRONT: (u8, u8, u8, u8) = (22, 26, 35, 255); // darker shelf front
const KEY_BASE: (u8, u8, u8, u8) = (32, 38, 51, 255); // tokens surface.card
const KEY_CAP: (u8, u8, u8, u8) = (124, 134, 156, 255);
const TEXT: (u8, u8, u8, u8) = (84, 72, 58, 255);
const FISH_BLUE: (u8, u8, u8) = (108, 160, 220);
// panel surfaces (dark-premium tokens): selected row is a raised card with a
// gold border (drawn separately); hover is a lifted control surface.
fn row_sel() -> (u8, u8, u8, u8) { tokens::surface_card() }
fn row_hover() -> (u8, u8, u8, u8) { tokens::surface_control_hover() }
const PIN_GOLD: (u8, u8, u8, u8) = tokens::ACCENT_GOLD;

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

/// A floating "+N XP" popup that drifts up and fades after an XP award
/// (nom +5, petting +10). `life` runs 1 → 0.
pub struct XpPop {
    pub x: f32,
    pub y: f32,
    pub life: f32,
    pub amount: u32,
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
    /// How many copies this fish represents. >1 when copies merged into it
    /// because the flight queue was full (drawn as a `+N` count badge).
    pub count: u32,
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
                count: 1,
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
            count: 1,
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
    /// Curious ear-flick rotation (degrees) and horizontal eye-glance (px);
    /// both 0 outside the Curious mood. See [`crate::pet::PetMood::Curious`].
    pub ear_twitch: f32,
    pub look: f32,
    pub accessory: Accessory,
    pub particles: &'a [Particle],
    pub xp_pops: &'a [XpPop],
    pub fish: Option<FishView<'a>>,
    pub bubble: Option<BubbleData>,
    pub bubble_alpha: f32,
    pub toast: Option<(&'a str, f32)>,
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

// ---- dark-premium primitives ----------------------------------------------
//
// Public, transform-aware draw helpers built on the geometry above and the
// [`crate::tokens`] palette. They take a `&mut Pixmap` plus the same
// scale+translate `Transform` the renderer uses, so the upcoming panel/pet
// restyle milestones share one rounded-rect / focus-ring / source-badge
// implementation instead of re-deriving them per call site.

/// A logical-coordinate rounded rectangle: `(x, y, w, h)` plus corner radius.
pub type RRect = (f32, f32, f32, f32);

/// Fill a rounded rectangle (logical coords) under `ts`.
pub fn fill_round_rect(pm: &mut Pixmap, rect: RRect, r: f32, c: tokens::Rgba, ts: Transform) {
    if let Some(p) = round_rect(rect.0, rect.1, rect.2, rect.3, r) {
        pm.fill_path(&p, &paint(c), FillRule::Winding, ts, None);
    }
}

/// Stroke a rounded rectangle (logical coords) under `ts`.
pub fn stroke_round_rect(pm: &mut Pixmap, rect: RRect, r: f32, c: tokens::Rgba, width: f32, ts: Transform) {
    if let Some(p) = round_rect(rect.0, rect.1, rect.2, rect.3, r) {
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };
        pm.stroke_path(&p, &paint(c), &stroke, ts, None);
    }
}

/// Gold focus ring (`border.focus` / `stroke.focus`) inset half a stroke so it
/// sits crisply inside the given rect — for selected rows and focused controls.
pub fn focus_border(pm: &mut Pixmap, rect: RRect, r: f32, ts: Transform) {
    let i = tokens::STROKE_FOCUS / 2.0;
    stroke_round_rect(
        pm,
        (rect.0 + i, rect.1 + i, rect.2 - 2.0 * i, rect.3 - 2.0 * i),
        (r - i).max(0.0),
        tokens::BORDER_FOCUS,
        tokens::STROKE_FOCUS,
        ts,
    );
}

/// Draw the per-app source identity badge — a `size`-wide rounded chip centered
/// at (`cx`, `cy`): the extracted app icon when present, otherwise the app
/// initial on the stable [`source_color`] chip. Shared by the fish and the
/// panel rows so one app always reads as one color (the spec's identity rule).
pub fn source_badge(pm: &mut Pixmap, cx: f32, cy: f32, size: f32, badge: &Badge, ts: Transform) {
    let r = size * 0.30;
    if let Some(icon) = &badge.icon {
        let chip = round_rect(cx - size / 2.0, cy - size / 2.0, size, size, r);
        if let Some(chip) = chip {
            pm.fill_path(&chip, &paint((255, 255, 255, 235)), FillRule::Winding, ts, None);
        }
        let k = size / icon.width() as f32;
        let it = ts.pre_translate(cx - size / 2.0, cy - size / 2.0).pre_scale(k, k);
        let pp = PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..Default::default()
        };
        pm.draw_pixmap(0, 0, icon.as_ref(), &pp, it, None);
    } else {
        let (r8, g8, b8) = badge.color;
        fill_round_rect(pm, (cx - size / 2.0, cy - size / 2.0, size, size), r, (r8, g8, b8, 255), ts);
        // sysfont's `px` is a multiplier whose cell height is ~7*px, so a letter
        // filling ~70% of the chip needs px ≈ size * 0.1.
        let px = size * 0.1;
        let label = badge.letter.to_string();
        let lw = sysfont::measure(&label, px);
        sysfont::draw(pm, &label, cx - lw / 2.0, cy - 3.5 * px, px, tokens::text_primary(), ts);
    }
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
    /// Pseudo-bold: the system-font rasterizer is single-weight, so thicken a
    /// heading by overprinting it with a sub-pixel horizontal offset.
    fn ui_text_b(&mut self, s: &str, x: f32, y: f32, px: f32, c: (u8, u8, u8, u8)) {
        sysfont::draw(self.pm, s, x, y, px, c, self.ts);
        sysfont::draw(self.pm, s, x + 0.45, y, px, c, self.ts);
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

/// Dark-premium background colour for the portable backend's opaque "card"
/// (tokens surface.window) — the cat floats on a dark surface like the
/// references, since softbuffer can't carry the desktop through (ADR-0003).
pub fn card_bg() -> (u8, u8, u8, u8) { tokens::surface_window() }

/// Render onto a fully transparent canvas — the native layered-window backend,
/// where transparent pixels let the desktop show through and are click-through.
pub fn render(pm: &mut Pixmap, sc: &Scene, scale: f32) {
    pm.fill(tiny_skia::Color::TRANSPARENT);
    draw_scene(pm, sc, scale);
}

/// Render onto an opaque rounded "card" — used by the portable backend, whose
/// pixel buffer (softbuffer) cannot carry per-pixel alpha to the compositor.
pub fn render_card(pm: &mut Pixmap, sc: &Scene, scale: f32) {
    let cb = card_bg();
    pm.fill(tiny_skia::Color::from_rgba8(cb.0, cb.1, cb.2, cb.3));
    let w = pm.width() as f32;
    let h = pm.height() as f32;
    {
        let mut cv = Cv {
            pm,
            ts: Transform::identity(),
        };
        // subtle inset frame so the window reads as a tidy little widget
        let frame = round_rect(3.0, 3.0, w - 6.0, h - 6.0, 14.0);
        cv.stroke(&frame, tokens::border_subtle(), 1.5);
    }
    draw_scene(pm, sc, scale);
}

fn draw_scene(pm: &mut Pixmap, sc: &Scene, scale: f32) {
    let mut cv = Cv {
        pm,
        ts: Transform::from_scale(scale, scale).pre_translate(sc.origin.0, sc.origin.1),
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

    // ears (under head outline) — a curious flick rotates them about the crown
    {
        let head_t = if sc.ear_twitch.abs() > 0.01 {
            Transform::from_rotate_at(sc.ear_twitch, 120.0, 92.0).post_concat(head_t)
        } else {
            head_t
        };
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
    cv.stroke(&top, tokens::border_subtle(), 1.5);

    // keyboard
    {
        let base = round_rect(74.0, 200.0, 92.0, 18.0, 5.0);
        cv.fill(&base, KEY_BASE);
        cv.stroke(&base, tokens::border_strong(), 1.5);
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

    // floating "+N XP" popups (gold with a dark halo, drifting up)
    for xp in sc.xp_pops {
        if xp.life > 0.01 {
            let a = xp.life.min(1.0);
            let label = format!("+{} XP", xp.amount);
            let pxs = 2.2;
            let w = sysfont::measure(&label, pxs);
            cv.ui_text(&label, xp.x - w / 2.0 + 0.5, xp.y + 0.5, pxs, fade((40, 30, 20, 255), a));
            cv.ui_text(&label, xp.x - w / 2.0, xp.y, pxs, fade(tokens::ACCENT_GOLD, a));
        }
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
            // big sparkly eyes while a fish is incoming; curious glance shifts
            // the pupil sideways
            let r = if chase { 6.2 } else { 5.2 };
            let px = ex + sc.look;
            cv.fill_t(&oval(px, 122.0, r, (ry * r / 5.2).max(0.8)), OUTLINE, t);
            if ry > 2.0 {
                cv.fill_t(&oval(px - 1.6, 120.2, 1.7, 1.7 * (ry / 5.2)), (255, 255, 255, 230), t);
            }
        }
    }

    if sc.mouth_open > 0.05 {
        // open mouth, ready to nom
        let o = sc.mouth_open.clamp(0.0, 1.0);
        let mouth = oval(120.0, 138.0, 4.5 + 3.5 * o, 3.0 + 5.5 * o);
        cv.fill_t(&mouth, (164, 88, 92, 255), t);
        cv.stroke_t(&mouth, OUTLINE, 2.2, t);
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
        cv.fill_t(&tail, dark_c, t);
        cv.stroke_t(&tail, OUTLINE, 2.4, t);
    }
    // body
    let body = oval(x, y, 16.0, 10.0);
    cv.fill_t(&body, body_c, t);
    cv.stroke_t(&body, OUTLINE, 2.6, t);
    // top fin
    {
        let mut pb = PathBuilder::new();
        pb.move_to(x - 4.0, y - 9.0);
        pb.quad_to(x + 1.0, y - 15.0, x + 7.0, y - 8.5);
        let fin = pb.finish();
        cv.fill_t(&fin, dark_c, t);
        cv.stroke_t(&fin, OUTLINE, 2.2, t);
    }
    // eye
    cv.fill_t(&oval(x - 10.0, y - 2.5, 1.9, 1.9), OUTLINE, t);

    // badge: real app icon if present, else letter chip
    if let Some(icon) = &f.badge.icon {
        let size = 13.0;
        let chip = round_rect(x - size / 2.0 - 1.5, y - size / 2.0 - 1.5, size + 3.0, size + 3.0, 4.0);
        cv.fill_t(&chip, (255, 255, 255, 235), t);
        cv.stroke_t(&chip, fade(OUTLINE, 0.7), 1.6, t);
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
        cv.fill_t(&chip, (255, 255, 255, 235), t);
        cv.stroke_t(&chip, fade(OUTLINE, 0.7), 1.6, t);
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

    // merged-copy count: "+N" floating above a fish that swallowed a burst.
    if f.badge.count > 1 {
        let label = format!("+{}", f.badge.count - 1);
        let px = 1.5 * f.scale;
        let s = cv.ts.sx;
        let lw = sysfont::measure(&label, px);
        let mut anchor = [tiny_skia::Point::from_xy(x, y - 13.0)];
        t.post_concat(cv.ts).map_points(&mut anchor);
        let base = Transform::from_scale(s, s)
            .post_translate(anchor[0].x - lw * s / 2.0, anchor[0].y - 3.5 * px * s);
        sysfont::draw(cv.pm, &label, 0.0, 0.0, px, (40, 30, 20, 210), base.post_translate(0.6, 0.6));
        sysfont::draw(cv.pm, &label, 0.0, 0.0, px, tokens::ACCENT_GOLD, base);
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
    let glass = fade(tokens::surface_panel(), a);
    let rect = round_rect(14.0, 2.0, 212.0, 84.0, 14.0);
    cv.fill(&rect, glass);
    cv.stroke(&rect, fade(tokens::border_strong(), a), 1.5);
    // tail pointing to the cat
    {
        let mut pb = PathBuilder::new();
        pb.move_to(112.0, 85.0);
        pb.line_to(128.0, 85.0);
        pb.line_to(121.0, 97.0);
        pb.close();
        let tail = pb.finish();
        cv.fill(&tail, glass);
        cv.stroke(&tail, fade(tokens::border_strong(), a), 1.5);
        // cover the seam
        if let Some(r) = Rect::from_xywh(113.5, 82.5, 13.0, 4.0) {
            cv.pm.fill_rect(r, &paint(glass), cv.ts, None);
        }
    }

    let tc = fade(tokens::text_primary(), a);
    // row 1: level + xp bar (gold, the design's progression accent)
    cv.ui_text(&format!("{} {}", t(lang, Msg::LevelShort), b.level), 24.0, 8.0, 2.0, tc);
    let bar_bg = round_rect(82.0, 8.5, 134.0, 12.0, 6.0);
    cv.fill(&bar_bg, fade(tokens::surface_control(), a));
    let w = (134.0 * b.pct.clamp(0.0, 1.0)).max(10.0);
    let bar_fg = round_rect(82.0, 8.5, w, 12.0, 6.0);
    cv.fill(&bar_fg, fade(tokens::ACCENT_GOLD, a));
    cv.stroke(&bar_bg, fade(tokens::border_subtle(), a), 1.5);

    // rows 2-5: today's keys / clicks / copies / active time
    let px = 1.7;
    let rows: [(Msg, String); 4] = [
        (Msg::BubbleKeys, fmt_thousands(b.keys)),
        (Msg::BubbleClicks, fmt_thousands(b.clicks)),
        (Msg::BubbleClips, fmt_thousands(b.copies)),
        (Msg::BubbleActive, {
            let (h, m) = (b.minutes / 60, b.minutes % 60);
            format!("{}H {:02}M", h, m)
        }),
    ];
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = 27.0 + i as f32 * 14.0;
        cv.ui_text(t(lang, *label), 24.0, y, px, fade(tokens::text_muted(), a));
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
pub fn draw_panel(pm: &mut Pixmap, v: &PanelView, scale: f32) {
    let mut cv = Cv {
        pm,
        ts: Transform::from_scale(scale, scale),
    };
    let lang = v.lang;
    let lt = v.panel.layout();

    // dark-premium text roles, reused throughout the panel
    let txt = tokens::text_primary();
    let txt2 = tokens::text_secondary();
    let muted = tokens::text_muted();

    // soft drop shadow + dark glass card
    cv.fill(
        &round_rect(lt.card_x - 2.0, lt.card_y + 3.0, lt.card_w + 4.0, lt.card_h + 4.0, 20.0),
        (5, 7, 12, 70),
    );
    let card = round_rect(lt.card_x, lt.card_y, lt.card_w, lt.card_h, 18.0);
    cv.fill(&card, tokens::surface_panel());
    cv.stroke(&card, tokens::border_strong(), 1.5);

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
        cv.fill(&fish, tokens::ACCENT_GOLD);
        cv.stroke(&fish, tokens::ACCENT_GOLD_2, 1.4);
        cv.line(
            &[
                (hx + 12.0, hcy),
                (hx + 16.5, hcy - 4.0),
                (hx + 16.5, hcy + 4.0),
                (hx + 12.0, hcy),
            ],
            tokens::ACCENT_GOLD_2,
            1.6,
        );
        cv.fill(&oval(hx + 4.5, hcy - 1.2, 1.0, 1.0), (30, 24, 12, 255));
    }
    cv.ui_text(t(lang, Msg::PanelTitle), hx + 22.0, hcy - 7.0, 2.0, txt);
    if !v.capture {
        // capture paused: small amber pause bars next to the title
        let tx = hx + 22.0 + sysfont::measure(t(lang, Msg::PanelTitle), 2.0) + 8.0;
        cv.fill(&round_rect(tx, hcy - 6.5, 3.2, 13.0, 1.3), tokens::ACCENT_GOLD_2);
        cv.fill(&round_rect(tx + 5.4, hcy - 6.5, 3.2, 13.0, 1.3), tokens::ACCENT_GOLD_2);
    }

    // header buttons
    draw_btn(&mut cv, lt.btn_filter_x, lt.btn_y, BtnIcon::Filter(v.panel.source.is_some()));
    draw_btn(&mut cv, lt.btn_pause_x, lt.btn_y, BtnIcon::Pause(v.capture));
    draw_btn(&mut cv, lt.btn_clear_x, lt.btn_y, BtnIcon::Trash(v.panel.clear_armed));
    draw_btn(&mut cv, lt.btn_lang_x, lt.btn_y, BtnIcon::Lang(lang));
    draw_btn(&mut cv, lt.btn_close_x, lt.btn_y, BtnIcon::Close);

    // search box
    let (sx, sy, sw, sh) = (lt.search_x, lt.search_y, lt.search_w, lt.search_h);
    let sb = round_rect(sx, sy, sw, sh, 9.0);
    cv.fill(&sb, tokens::surface_control());
    cv.stroke(&sb, tokens::border_subtle(), 1.5);
    // magnifier
    let scy = sy + sh / 2.0;
    cv.stroke(&oval(sx + 10.0, scy - 1.5, 3.6, 3.6), muted, 1.8);
    cv.line(&[(sx + 13.2, scy + 1.5), (sx + 16.2, scy + 4.5)], muted, 1.8);
    let mut qx = sx + 23.0;
    // active source filter: a chip inside the search box; query starts after
    if let Some(src) = &v.panel.source {
        let dot = source_color(src);
        let label = sysfont::truncate_to_width(src, 1.4, 90.0);
        let cw = sysfont::measure(&label, 1.4) + 18.0;
        let chip = round_rect(qx - 2.0, sy + 3.0, cw, sh - 6.0, 7.0);
        cv.fill(&chip, tokens::surface_card());
        cv.stroke(&chip, tokens::border_strong(), 1.0);
        cv.fill(&oval(qx + 4.5, scy, 2.6, 2.6), (dot.0, dot.1, dot.2, 255));
        cv.ui_text(&label, qx + 10.0, sy + 5.0, 1.4, txt2);
        qx += cw + 4.0;
    }
    let qmax = sx + sw - 8.0 - qx;
    let qpx = 1.8;
    let qy = sy + (sh - 7.0 * qpx) / 2.0;
    if v.panel.query.is_empty() {
        cv.ui_text(t(lang, Msg::SearchHint), qx, qy, qpx, muted);
        if v.caret {
            cv.fill(&round_rect(qx - 3.0, sy + 3.0, 1.6, sh - 6.0, 0.8), tokens::ACCENT_GOLD);
        }
    } else {
        // show the tail of long queries
        let mut q: &str = &v.panel.query;
        while sysfont::measure(q, qpx) > qmax - 6.0 {
            let mut it = q.chars();
            it.next();
            q = it.as_str();
        }
        cv.ui_text(q, qx, qy, qpx, txt);
        if v.caret {
            let cx = qx + sysfont::measure(q, qpx) + 2.0;
            cv.fill(&round_rect(cx, sy + 3.0, 1.6, sh - 6.0, 0.8), tokens::ACCENT_GOLD);
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
            lt.rows_y + pl::ROW_H * lt.rows as f32 / 2.0 - 8.0,
            1.8,
            muted,
        );
    }
    let hover = v.panel.cursor.filter(|(x, _)| {
        (lt.row_x..=lt.row_x + lt.row_w).contains(x)
    });
    let hover_row = hover.and_then(|(_, y)| v.panel.row_at(y, total));
    let now = crate::clipboard::now_ts();
    for i in 0..lt.rows {
        let idx = v.panel.scroll + i;
        let Some(clip) = visible.get(idx) else { break };
        let ry = lt.rows_y + i as f32 * pl::ROW_H;

        let row_bg = match (idx == v.panel.sel, hover_row == Some(idx)) {
            (true, true) => Some(tokens::surface_control_active()), // selected-hover
            (true, false) => Some(row_sel()),
            (false, true) => Some(row_hover()),
            (false, false) => None,
        };
        if let Some(bg) = row_bg {
            let rrect = (lt.row_x, ry + 1.0, lt.row_w, pl::ROW_H - 2.0);
            cv.fill(&round_rect(rrect.0, rrect.1, rrect.2, rrect.3, 8.0), bg);
            // selected row gets the gold focus border (design: raised + gold)
            if idx == v.panel.sel {
                focus_border(cv.pm, rrect, 8.0, cv.ts);
            }
        }

        // pin star
        let star = star_path(lt.row_x + 12.0, ry + pl::ROW_H / 2.0, 6.5, 0.0);
        if clip.pinned {
            cv.fill(&star, PIN_GOLD);
            cv.stroke(&star, tokens::ACCENT_GOLD_2, 1.4);
        } else {
            cv.stroke(&star, fade(muted, 0.7), 1.4);
        }

        // quick-copy badge: the first ten rows answer to Ctrl+0..9
        let quick = idx < pl::QUICK_KEYS;
        let badge_w = if quick { 18.0 } else { 0.0 };

        // preview + meta (source dot + app - age - size)
        let tx = lt.row_x + 28.0;
        let tmax = lt.row_x + lt.row_w - pl::DEL_ZONE - tx - 4.0 - badge_w;
        let prev = sysfont::truncate_to_width(&clip.preview(), 1.9, tmax);
        cv.ui_text(&prev, tx, ry + 4.5, 1.9, txt);
        let mut meta = i18n::time_ago(lang, now.saturating_sub(clip.ts));
        if clip.text.len() > 500 {
            meta = format!("{meta} - {:.1}K", clip.text.len() as f32 / 1024.0);
        }
        let mut mx = tx;
        if let Some(src) = &clip.source {
            // the same colored-initial identity badge the fish wears, so an app
            // reads as one color across the fish and its rows
            source_badge(cv.pm, mx + 4.5, ry + 25.0, 9.0, &Badge::from_source(Some(src)), cv.ts);
            mx += 13.0;
            meta = format!("{src} - {meta}");
        }
        let meta = sysfont::truncate_to_width(&meta, 1.35, tmax - (mx - tx));
        cv.ui_text(&meta, mx, ry + 20.5, 1.35, muted);

        if quick {
            // purple quick-copy digit badge (Ctrl+0..9), matching the fish/row
            // identity language of the design references
            let qx = lt.row_x + lt.row_w - pl::DEL_ZONE - 16.0;
            cv.fill(&oval(qx + 6.5, ry + pl::ROW_H / 2.0, 7.0, 7.0), tokens::ACCENT_PURPLE);
            let d = char::from(b'0' + idx as u8).to_string();
            let dw = sysfont::measure(&d, 1.3);
            cv.ui_text(&d, qx + 6.5 - dw / 2.0, ry + pl::ROW_H / 2.0 - 4.6, 1.3, tokens::text_primary());
        }

        // delete x (red halo while hovered, so a destructive click is obvious)
        let dx = lt.row_x + lt.row_w - 14.0;
        let dy = ry + pl::ROW_H / 2.0;
        let on_del = hover_row == Some(idx)
            && hover.is_some_and(|(x, _)| x > lt.row_x + lt.row_w - pl::DEL_ZONE);
        let xc = if on_del {
            cv.fill(&oval(dx, dy, 8.0, 8.0), fade(tokens::ACCENT_RED, 0.22));
            tokens::ACCENT_RED
        } else {
            fade(muted, 0.8)
        };
        cv.line(&[(dx - 3.4, dy - 3.4), (dx + 3.4, dy + 3.4)], xc, 1.8);
        cv.line(&[(dx - 3.4, dy + 3.4), (dx + 3.4, dy - 3.4)], xc, 1.8);

        // separator
        if i + 1 < lt.rows {
            cv.line(
                &[(lt.row_x + 4.0, ry + pl::ROW_H), (lt.row_x + lt.row_w - 4.0, ry + pl::ROW_H)],
                tokens::border_subtle(),
                1.0,
            );
        }
    }

    // scrollbar
    if total > lt.rows {
        let track_h = pl::ROW_H * lt.rows as f32 - 4.0;
        let tx = lt.card_x + lt.card_w - 7.0;
        cv.fill(&round_rect(tx, lt.rows_y + 2.0, 3.5, track_h, 1.7), tokens::border_subtle());
        let th = (track_h * lt.rows as f32 / total as f32).max(16.0);
        let ty = lt.rows_y + 2.0
            + (track_h - th) * v.panel.scroll as f32 / (total - lt.rows) as f32;
        cv.fill(&round_rect(tx, ty, 3.5, th, 1.7), fade((255, 255, 255, 255), 0.32));
    }

    // footer: count + hotkey hint, then a keyboard-shortcut help line
    cv.line(
        &[(lt.row_x, lt.footer_y - 1.0), (lt.row_x + lt.row_w, lt.footer_y - 1.0)],
        tokens::border_subtle(),
        1.0,
    );
    let count = i18n::clip_count(lang, v.store.len(), v.store.pinned_count());
    cv.ui_text(&count, lt.card_x + 10.0, lt.footer_y + 3.0, 1.4, txt2);
    let hw = sysfont::measure(v.hint, 1.4);
    cv.ui_text(
        v.hint,
        lt.card_x + lt.card_w - 10.0 - hw,
        lt.footer_y + 3.0,
        1.4,
        tokens::ACCENT_GOLD,
    );
    let keys = sysfont::truncate_to_width(t(lang, Msg::FooterKeys), 1.25, lt.row_w);
    let kw = sysfont::measure(&keys, 1.25);
    cv.ui_text(
        &keys,
        lt.card_x + (lt.card_w - kw) / 2.0,
        lt.footer_y + 16.0,
        1.25,
        muted,
    );

    // resize grip: three diagonal score lines in the bottom-right corner
    {
        let (gx, gy) = (lt.card_x + lt.card_w, lt.card_y + lt.card_h);
        for i in 0..3 {
            let d = 5.0 + i as f32 * 4.0;
            cv.line(&[(gx - d, gy - 3.0), (gx - 3.0, gy - d)], fade((255, 255, 255, 255), 0.32), 1.6);
        }
    }
}

enum BtnIcon {
    Filter(bool), // source filter currently active?
    Pause(bool),  // capture currently on?
    Trash(bool),  // clear armed (next press clears)?
    Lang(Lang),
    Close,
}

fn draw_btn(cv: &mut Cv, bx: f32, by: f32, icon: BtnIcon) {
    let b = pl::BTN;
    let bg = round_rect(bx, by, b, b, 8.0);
    let icon_c;
    if matches!(icon, BtnIcon::Pause(false) | BtnIcon::Trash(true)) {
        cv.fill(&bg, fade(tokens::ACCENT_RED, 0.18));
        cv.stroke(&bg, tokens::ACCENT_RED, 1.5);
        icon_c = tokens::ACCENT_RED;
    } else if matches!(icon, BtnIcon::Filter(true)) {
        cv.fill(&bg, fade(tokens::ACCENT_BLUE, 0.20));
        cv.stroke(&bg, tokens::ACCENT_BLUE, 1.5);
        icon_c = tokens::ACCENT_BLUE;
    } else {
        cv.fill(&bg, tokens::surface_control());
        cv.stroke(&bg, tokens::border_subtle(), 1.5);
        icon_c = tokens::text_secondary();
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
                cv.fill(&funnel, icon_c);
            } else {
                cv.stroke(&funnel, icon_c, 1.5);
            }
        }
        BtnIcon::Pause(true) => {
            // capture running -> show pause bars
            cv.fill(&round_rect(cx - 4.0, cy - 4.5, 2.9, 9.0, 1.2), icon_c);
            cv.fill(&round_rect(cx + 1.1, cy - 4.5, 2.9, 9.0, 1.2), icon_c);
        }
        BtnIcon::Pause(false) => {
            // paused -> show play triangle
            let mut pb = PathBuilder::new();
            pb.move_to(cx - 3.1, cy - 4.7);
            pb.line_to(cx + 4.5, cy);
            pb.line_to(cx - 3.1, cy + 4.7);
            pb.close();
            cv.fill(&pb.finish(), icon_c);
        }
        BtnIcon::Trash(armed) => {
            let c = icon_c;
            let _ = armed;
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
            sysfont::draw(cv.pm, s, cx - w / 2.0, cy - 4.3, 1.25, icon_c, cv.ts);
        }
        BtnIcon::Close => {
            cv.line(&[(cx - 4.0, cy - 4.0), (cx + 4.0, cy + 4.0)], icon_c, 2.0);
            cv.line(&[(cx - 4.0, cy + 4.0), (cx + 4.0, cy - 4.0)], icon_c, 2.0);
        }
    }
}

// ---- expanded three-pane screen (ADR-0012) ---------------------------------

/// Everything the expanded "full screen" needs to draw one frame: the panel
/// state + store (shared with the compact panel) plus the sidebar's live
/// progression/activity figures.
pub struct ExpandedView<'a> {
    pub panel: &'a Panel,
    pub store: &'a ClipStore,
    pub lang: Lang,
    pub version: &'a str,
    pub capture: bool,
    pub caret: bool,
    pub level: u32,
    pub xp_into: u64,
    pub xp_need: u64,
    pub keys: u64,
    pub clicks: u64,
    pub copies: u64,
    pub autoclose: bool,
}

/// Draws the expanded three-pane screen (sidebar | clip list | detail),
/// matching the design package's full-app concept.
pub fn draw_expanded_panel(pm: &mut Pixmap, v: &ExpandedView, scale: f32) {
    let mut cv = Cv { pm, ts: Transform::from_scale(scale, scale) };
    let el = v.panel.expanded_layout();
    let (cx, cy, cw, ch) = el.card;

    // shadow + dark glass card
    cv.fill(&round_rect(cx - 2.0, cy + 3.0, cw + 4.0, ch + 4.0, 22.0), (5, 7, 12, 80));
    cv.fill(&round_rect(cx, cy, cw, ch, 20.0), tokens::surface_panel());
    cv.stroke(&round_rect(cx, cy, cw, ch, 20.0), tokens::border_strong(), 1.5);

    draw_exp_sidebar(&mut cv, v, &el);
    draw_exp_list(&mut cv, v, &el);
    draw_detail_pane(&mut cv, v, &el, crate::clipboard::now_ts());

    // collapse button (top-right): inward chevrons
    let (bx, by, bw, bh) = el.collapse;
    fill_round_rect(cv.pm, (bx, by, bw, bh), 6.0, tokens::surface_control(), cv.ts);
    stroke_round_rect(cv.pm, (bx, by, bw, bh), 6.0, tokens::border_subtle(), 1.5, cv.ts);
    let (mx, my) = (bx + bw / 2.0, by + bh / 2.0);
    cv.line(&[(mx - 4.5, my - 4.0), (mx - 1.0, my), (mx - 4.5, my + 4.0)], tokens::text_secondary(), 1.7);
    cv.line(&[(mx + 4.5, my - 4.0), (mx + 1.0, my), (mx + 4.5, my + 4.0)], tokens::text_secondary(), 1.7);
    let _ = v.caret;
}

fn draw_exp_sidebar(cv: &mut Cv, v: &ExpandedView, el: &pl::ExpandedLayout) {
    let lang = v.lang;
    let (txt, txt2, muted) = (tokens::text_primary(), tokens::text_secondary(), tokens::text_muted());
    let (sx, sy, sw, sh) = el.sidebar;
    cv.fill(&round_rect(sx, sy, sw, sh, 18.0), tokens::surface_window());
    cv.fill(&round_rect(sx + sw - 18.0, sy, 18.0, sh, 0.0), tokens::surface_window());
    cv.line(&[(sx + sw, sy + 6.0), (sx + sw, sy + sh - 6.0)], tokens::border_subtle(), 1.0);

    // logo row: gold fish mark + ClipCat + version
    let (lx, ly) = (sx + 16.0, sy + 18.0);
    {
        let mut pb = PathBuilder::new();
        pb.move_to(lx + 11.0, ly);
        pb.quad_to(lx + 5.5, ly - 5.0, lx + 0.5, ly);
        pb.quad_to(lx + 5.5, ly + 5.0, lx + 11.0, ly);
        pb.close();
        cv.fill(&pb.finish(), tokens::ACCENT_GOLD);
        cv.line(&[(lx + 11.0, ly), (lx + 15.0, ly - 3.5), (lx + 15.0, ly + 3.5), (lx + 11.0, ly)], tokens::ACCENT_GOLD_2, 1.5);
    }
    cv.ui_text_b("ClipCat", lx + 20.0, ly - 6.5, 1.9, txt);
    let vw = sysfont::measure(v.version, 1.1);
    cv.ui_text(v.version, sx + sw - 14.0 - vw, ly - 4.0, 1.1, muted);

    // avatar card: mini cat on the left, level ring on the right
    let av = (sx + 14.0, sy + 34.0, sw - 28.0, 60.0);
    fill_round_rect(cv.pm, av, 14.0, tokens::surface_card(), cv.ts);
    mini_cat(cv, av.0 + 34.0, av.1 + 30.0);
    let ring_cx = av.0 + av.2 - 26.0;
    let ring_cy = av.1 + 30.0;
    let pct = if v.xp_need > 0 { v.xp_into as f32 / v.xp_need as f32 } else { 0.0 };
    progress_ring(cv, ring_cx, ring_cy, 16.0, pct);
    let lv = format!("Lv. {}", v.level);
    let lw = sysfont::measure(&lv, 1.15);
    cv.ui_text_b(&lv, ring_cx - lw / 2.0, ring_cy - 4.0, 1.15, txt);

    // XP text + bar
    let xpline = format!("{} / {} XP", fmt_thousands(v.xp_into), fmt_thousands(v.xp_need));
    cv.ui_text(&xpline, sx + 16.0, sy + 100.0, 1.3, txt2);
    let xpb = (sx + 16.0, sy + 116.0, sw - 32.0, 7.0);
    fill_round_rect(cv.pm, xpb, 3.5, tokens::surface_control(), cv.ts);
    fill_round_rect(cv.pm, (xpb.0, xpb.1, (xpb.2 * pct.clamp(0.0, 1.0)).max(4.0), xpb.3), 3.5, tokens::ACCENT_GOLD, cv.ts);

    // three stat cards (keys / clicks / copies) with colored icons
    let stats = [
        (fmt_thousands(v.keys), Msg::BubbleKeys, tokens::ACCENT_PURPLE),
        (fmt_thousands(v.clicks), Msg::BubbleClicks, tokens::ACCENT_GREEN),
        (fmt_thousands(v.copies), Msg::BubbleClips, tokens::ACCENT_BLUE),
    ];
    let gap = 6.0;
    let cw3 = (sw - 32.0 - 2.0 * gap) / 3.0;
    for (i, (value, label, color)) in stats.iter().enumerate() {
        let bx = sx + 16.0 + i as f32 * (cw3 + gap);
        let by = sy + 132.0;
        fill_round_rect(cv.pm, (bx, by, cw3, 50.0), 10.0, tokens::surface_card(), cv.ts);
        stat_icon(cv, *label, bx + cw3 / 2.0, by + 13.0, *color);
        let vwi = sysfont::measure(value, 1.4);
        cv.ui_text_b(value, bx + (cw3 - vwi) / 2.0, by + 23.0, 1.4, txt);
        let lab = t(lang, *label);
        let lwi = sysfont::measure(lab, 1.05);
        cv.ui_text(lab, bx + (cw3 - lwi) / 2.0, by + 37.0, 1.05, muted);
    }

    // nav items with icons + count badges
    for (i, nav) in pl::NavView::ALL.iter().enumerate() {
        let y = el.nav_y0 + i as f32 * el.nav_h;
        let selected = *nav == v.panel.nav;
        if selected {
            fill_round_rect(cv.pm, (sx + 10.0, y, sw - 20.0, el.nav_h - 4.0), 8.0, fade(tokens::ACCENT_GOLD, 0.14), cv.ts);
            fill_round_rect(cv.pm, (sx + 10.0, y + 5.0, 3.0, el.nav_h - 14.0), 1.5, tokens::ACCENT_GOLD, cv.ts);
        }
        let icol = if selected { tokens::ACCENT_GOLD } else { txt2 };
        nav_icon(cv, *nav, sx + 24.0, y + (el.nav_h - 4.0) / 2.0, icol);
        let ny = y + (el.nav_h - 4.0) / 2.0 - 5.5;
        if selected {
            cv.ui_text_b(t(lang, nav_msg(*nav)), sx + 38.0, ny, 1.6, tokens::ACCENT_GOLD);
        } else {
            cv.ui_text(t(lang, nav_msg(*nav)), sx + 38.0, ny, 1.6, txt);
        }
        if let Some(n) = nav_count(*nav, v.store) {
            let s = n.to_string();
            let bw = sysfont::measure(&s, 1.2) + 12.0;
            let chip = (sx + sw - 16.0 - bw, y + (el.nav_h - 4.0) / 2.0 - 7.0, bw, 14.0);
            let cc = if selected { fade(tokens::ACCENT_GOLD, 0.22) } else { tokens::surface_control() };
            fill_round_rect(cv.pm, chip, 7.0, cc, cv.ts);
            cv.ui_text(&s, chip.0 + 6.0, chip.1 + 1.0, 1.2, if selected { tokens::ACCENT_GOLD } else { muted });
        }
    }

    // auto-close panel toggle card
    let ac = (sx + 14.0, sy + sh - 104.0, sw - 28.0, 36.0);
    fill_round_rect(cv.pm, ac, 10.0, tokens::surface_card(), cv.ts);
    cv.ui_text(t(lang, Msg::MenuAutoClose), ac.0 + 12.0, ac.1 + 8.0, 1.3, txt);
    cv.ui_text(t(lang, Msg::ExpAfterCopy), ac.0 + 12.0, ac.1 + 22.0, 1.05, muted);
    toggle_switch(cv, el.autoclose_toggle, v.autoclose);

    // capture status card + pause/resume button
    let cc = (sx + 14.0, sy + sh - 60.0, sw - 28.0, 36.0);
    fill_round_rect(cv.pm, cc, 10.0, tokens::surface_card(), cv.ts);
    let (dot, msg) = if v.capture {
        (tokens::ACCENT_GREEN, Msg::CaptureRunning)
    } else {
        (tokens::ACCENT_RED, Msg::CapturePaused)
    };
    cv.fill(&oval(cc.0 + 14.0, cc.1 + 14.0, 3.0, 3.0), dot);
    cv.ui_text(t(lang, Msg::MenuCapture), cc.0 + 24.0, cc.1 + 8.0, 1.3, txt);
    cv.ui_text(t(lang, msg), cc.0 + 24.0, cc.1 + 22.0, 1.05, if v.capture { tokens::ACCENT_GREEN } else { tokens::ACCENT_RED });
    let cb = el.capture_btn;
    fill_round_rect(cv.pm, cb, 7.0, tokens::surface_control(), cv.ts);
    let (bcx, bcy) = (cb.0 + cb.2 / 2.0, cb.1 + cb.3 / 2.0);
    if v.capture {
        cv.fill(&round_rect(bcx - 3.5, bcy - 4.0, 2.6, 8.0, 1.0), txt2);
        cv.fill(&round_rect(bcx + 1.0, bcy - 4.0, 2.6, 8.0, 1.0), txt2);
    } else {
        let mut pb = PathBuilder::new();
        pb.move_to(bcx - 3.0, bcy - 4.0);
        pb.line_to(bcx + 4.0, bcy);
        pb.line_to(bcx - 3.0, bcy + 4.0);
        pb.close();
        cv.fill(&pb.finish(), tokens::ACCENT_GREEN);
    }

    // footer links
    cv.ui_text("GitHub   ·   Help   ·   About", sx + 16.0, sy + sh - 18.0, 1.1, muted);
}

fn draw_exp_list(cv: &mut Cv, v: &ExpandedView, el: &pl::ExpandedLayout) {
    let lang = v.lang;
    let (txt, txt2, muted) = (tokens::text_primary(), tokens::text_secondary(), tokens::text_muted());
    let rows = v.panel.expanded_visible(v.store);
    let (lxc, lyc, lwc, _) = el.list;
    let now = crate::clipboard::now_ts();

    // toolbar: source-filter chip + clear-unpinned + clip count
    let tf = el.toolbar_filter;
    fill_round_rect(cv.pm, tf, 8.0, tokens::surface_control(), cv.ts);
    stroke_round_rect(cv.pm, tf, 8.0, tokens::border_subtle(), 1.2, cv.ts);
    let flabel = match &v.panel.source {
        Some(s) => sysfont::truncate_to_width(s, 1.3, tf.2 - 24.0),
        None => sysfont::truncate_to_width(t(lang, Msg::AllSources), 1.3, tf.2 - 24.0),
    };
    cv.ui_text(&flabel, tf.0 + 10.0, tf.1 + (tf.3 - 9.1) / 2.0, 1.3, txt2);
    // caret ▾
    let (fcx, fcy) = (tf.0 + tf.2 - 11.0, tf.1 + tf.3 / 2.0);
    cv.line(&[(fcx - 3.0, fcy - 1.5), (fcx, fcy + 2.0), (fcx + 3.0, fcy - 1.5)], muted, 1.4);
    // clear-unpinned (trash) — turns red when armed
    let tc = el.toolbar_clear;
    let armed = v.panel.clear_armed;
    fill_round_rect(cv.pm, tc, 8.0, if armed { fade(tokens::ACCENT_RED, 0.18) } else { tokens::surface_control() }, cv.ts);
    stroke_round_rect(cv.pm, tc, 8.0, if armed { tokens::ACCENT_RED } else { tokens::border_subtle() }, 1.2, cv.ts);
    trash_icon(cv, tc.0 + tc.2 / 2.0, tc.1 + tc.3 / 2.0, if armed { tokens::ACCENT_RED } else { txt2 });
    // count, right-aligned
    let count = i18n::clip_count(lang, v.store.len(), v.store.pinned_count());
    let cwi = sysfont::measure(&count, 1.25);
    cv.ui_text(&count, lxc + lwc - 14.0 - cwi, tf.1 + (tf.3 - 8.75) / 2.0, 1.25, muted);

    // search/filter input
    let (qx, qy, qw, qh) = el.search;
    fill_round_rect(cv.pm, (qx, qy, qw, qh), 9.0, tokens::surface_control(), cv.ts);
    stroke_round_rect(cv.pm, (qx, qy, qw, qh), 9.0, tokens::border_subtle(), 1.5, cv.ts);
    cv.stroke(&oval(qx + 11.0, qy + qh / 2.0 - 1.5, 3.4, 3.4), muted, 1.7);
    cv.line(&[(qx + 14.0, qy + qh / 2.0 + 1.5), (qx + 17.0, qy + qh / 2.0 + 4.5)], muted, 1.7);
    if v.panel.query.is_empty() {
        cv.ui_text(t(lang, Msg::SearchHint), qx + 22.0, qy + (qh - 12.6) / 2.0, 1.7, muted);
    } else {
        cv.ui_text(&v.panel.query, qx + 22.0, qy + (qh - 12.6) / 2.0, 1.7, txt);
    }

    let hover_row = v.panel.cursor.and_then(|(hx, hy)| {
        if hx >= lxc && hx <= lxc + lwc { row_at_expanded(el, hy) } else { None }
    });
    if rows.is_empty() {
        let msg = if v.store.is_empty() { t(lang, Msg::PanelEmpty) } else { t(lang, Msg::PanelNoMatch) };
        let w = sysfont::measure(msg, 1.8);
        cv.ui_text(msg, lxc + (lwc - w) / 2.0, el.rows_y + 60.0, 1.8, muted);
    }
    for i in 0..el.rows {
        let idx = v.panel.scroll + i;
        let Some(clip) = rows.get(idx) else { break };
        let ry = el.rows_y + i as f32 * pl::ROW_H;
        let rrect = (lxc + 8.0, ry + 1.0, lwc - 16.0, pl::ROW_H - 3.0);
        if idx == v.panel.sel {
            fill_round_rect(cv.pm, rrect, 8.0, tokens::surface_card(), cv.ts);
            focus_border(cv.pm, rrect, 8.0, cv.ts);
        } else if hover_row == Some(idx) {
            fill_round_rect(cv.pm, rrect, 8.0, tokens::surface_control_hover(), cv.ts);
        }
        // left index badge (purple, top 10) else a pin star
        let cyr = ry + pl::ROW_H / 2.0;
        if idx < pl::QUICK_KEYS {
            cv.fill(&oval(lxc + 24.0, cyr, 8.0, 8.0), tokens::ACCENT_PURPLE);
            let d = char::from(b'0' + idx as u8).to_string();
            let dw = sysfont::measure(&d, 1.35);
            cv.ui_text(&d, lxc + 24.0 - dw / 2.0, cyr - 4.7, 1.35, tokens::text_primary());
        } else {
            let star = star_path(lxc + 24.0, cyr, 6.0, 0.0);
            if clip.pinned { cv.fill(&star, PIN_GOLD); } else { cv.stroke(&star, fade(muted, 0.6), 1.3); }
        }
        if idx < pl::QUICK_KEYS && clip.pinned {
            cv.fill(&star_path(lxc + 38.0, ry + 8.0, 4.0, 0.0), PIN_GOLD);
        }
        // title + preview (two lines), right reserved for source/time
        let tx = lxc + 40.0;
        let right_w = 84.0;
        let tmax = lwc - 16.0 - 40.0 - right_w;
        let mut lines = clip.text.lines();
        let title = sysfont::truncate_to_width(lines.next().unwrap_or("").trim(), 1.6, tmax);
        cv.ui_text_b(&title, tx, ry + 5.0, 1.6, txt);
        if let Some(second) = lines.next() {
            let s = sysfont::truncate_to_width(second.trim(), 1.25, tmax);
            cv.ui_text(&s, tx, ry + 20.0, 1.25, muted);
        }
        // right: source badge + app name (top) and time (below), right-aligned
        let rxr = lxc + lwc - 16.0;
        let age = i18n::time_ago(lang, now.saturating_sub(clip.ts));
        let aw = sysfont::measure(&age, 1.2);
        cv.ui_text(&age, rxr - aw, ry + 20.0, 1.2, muted);
        if let Some(src) = &clip.source {
            let name = sysfont::truncate_to_width(src, 1.25, right_w - 14.0);
            let nw = sysfont::measure(&name, 1.25);
            cv.ui_text(&name, rxr - nw, ry + 6.0, 1.25, txt2);
            source_badge(cv.pm, rxr - nw - 9.0, ry + 9.0, 9.0, &Badge::from_source(Some(src)), cv.ts);
        }
    }

    // pagination bar (visual): ‹ 1 2 3 … N ›
    let total = rows.len();
    let pages = total.div_ceil(el.rows.max(1)).max(1);
    let cur = v.panel.scroll / el.rows.max(1) + 1;
    draw_pagination(cv, lxc, lyc + el.list.3 - 26.0, lwc, pages, cur);
}

/// Pagination row centered in the list footer.
fn draw_pagination(cv: &mut Cv, lx: f32, y: f32, lw: f32, pages: usize, cur: usize) {
    let (txt2, muted) = (tokens::text_secondary(), tokens::text_muted());
    let mut labels: Vec<String> = Vec::new();
    labels.push("‹".into());
    let show: Vec<usize> = if pages <= 4 {
        (1..=pages).collect()
    } else {
        vec![1, 2, 3, pages]
    };
    for (i, p) in show.iter().enumerate() {
        if i == 3 && pages > 4 {
            labels.push("…".into());
        }
        labels.push(p.to_string());
    }
    labels.push("›".into());
    let widths: Vec<f32> = labels.iter().map(|s| sysfont::measure(s, 1.3).max(8.0) + 12.0).collect();
    let total_w: f32 = widths.iter().sum();
    let mut x = lx + (lw - total_w) / 2.0;
    for (lab, w) in labels.iter().zip(&widths) {
        let active = lab.parse::<usize>().ok() == Some(cur);
        if active {
            stroke_round_rect(cv.pm, (x, y, *w, 22.0), 7.0, tokens::ACCENT_GOLD, 1.5, cv.ts);
        }
        let tw = sysfont::measure(lab, 1.3);
        cv.ui_text(lab, x + (w - tw) / 2.0, y + 6.5, 1.3, if active { tokens::ACCENT_GOLD } else { txt2 });
        x += w;
    }
    let _ = muted;
}

fn draw_detail_pane(cv: &mut Cv, v: &ExpandedView, el: &pl::ExpandedLayout, now: u64) {
    let lang = v.lang;
    let (dx, dy, dw, dh) = el.detail;
    let (txt, txt2, muted) = (tokens::text_primary(), tokens::text_secondary(), tokens::text_muted());
    cv.line(&[(dx, dy + 6.0), (dx, dy + dh - 6.0)], tokens::border_subtle(), 1.0);
    let px = dx + 14.0;
    let pw = dw - 28.0;
    let rows = v.panel.expanded_visible(v.store);

    let Some(clip) = rows.get(v.panel.sel) else {
        let m = t(lang, Msg::DetailEmpty);
        let w = sysfont::measure(m, 1.5);
        cv.ui_text(m, dx + (dw - w) / 2.0, dy + dh / 2.0 - 6.0, 1.5, muted);
        return;
    };

    // title + pin star
    let title = sysfont::truncate_to_width(clip.preview().lines().next().unwrap_or("").trim(), 1.9, pw - 18.0);
    cv.ui_text_b(&title, px, dy + 14.0, 1.9, txt);
    let star = star_path(dx + dw - 16.0, dy + 20.0, 6.5, 0.0);
    if clip.pinned { cv.fill(&star, PIN_GOLD); } else { cv.stroke(&star, fade(muted, 0.7), 1.3); }

    // source + time + size line
    let mut sxs = px;
    if let Some(src) = &clip.source {
        source_badge(cv.pm, px + 4.5, dy + 36.0, 9.0, &Badge::from_source(Some(src)), cv.ts);
        cv.ui_text(src, px + 13.0, dy + 32.0, 1.25, txt2);
        sxs = px + 13.0 + sysfont::measure(src, 1.25) + 8.0;
    }
    let meta = format!("{} · {} B", i18n::time_ago(lang, now.saturating_sub(clip.ts)), clip.text.len());
    cv.ui_text(&meta, sxs, dy + 32.0, 1.2, muted);

    // code/preview box
    let box_y = dy + 50.0;
    let box_h = 96.0;
    fill_round_rect(cv.pm, (px, box_y, pw, box_h), 10.0, tokens::surface_window(), cv.ts);
    stroke_round_rect(cv.pm, (px, box_y, pw, box_h), 10.0, tokens::border_subtle(), 1.0, cv.ts);
    for (i, line) in clip.text.lines().take(5).enumerate() {
        let l = sysfont::truncate_to_width(line, 1.4, pw - 16.0);
        // first line bright, rest tinted like console output
        let c = if i == 0 { txt } else { tokens::ACCENT_GREEN };
        cv.ui_text(&l, px + 8.0, box_y + 9.0 + i as f32 * 15.0, 1.4, c);
    }

    // action grid (3 columns x 2 rows)
    let pin_label = if clip.pinned { Msg::ActUnpin } else { Msg::ActPin };
    let quick = if v.panel.sel < pl::QUICK_KEYS { format!("{}", v.panel.sel) } else { String::new() };
    let specs: [(Msg, &str, ExpKind); 6] = [
        (Msg::ActCopy, "Enter", ExpKind::Primary),
        (Msg::ActQuickCopy, &quick, ExpKind::Normal),
        (pin_label, "Ctrl P", ExpKind::Normal),
        (Msg::ActEditNote, "", ExpKind::Muted),
        (Msg::ActDelete, "Del", ExpKind::Danger),
        (Msg::ActOpenSource, "", ExpKind::Muted),
    ];
    let rects = v.panel.expanded_action_rects();
    for ((label, hint, kind), r) in specs.iter().zip(rects) {
        let fg = match kind {
            ExpKind::Primary => {
                fill_round_rect(cv.pm, r, 8.0, tokens::ACCENT_GOLD, cv.ts);
                (30, 24, 12, 255)
            }
            ExpKind::Danger => {
                fill_round_rect(cv.pm, r, 8.0, tokens::surface_control(), cv.ts);
                stroke_round_rect(cv.pm, r, 8.0, fade(tokens::ACCENT_RED, 0.6), 1.4, cv.ts);
                tokens::ACCENT_RED
            }
            ExpKind::Muted => {
                fill_round_rect(cv.pm, r, 8.0, tokens::surface_control(), cv.ts);
                stroke_round_rect(cv.pm, r, 8.0, tokens::border_subtle(), 1.2, cv.ts);
                tokens::text_muted()
            }
            ExpKind::Normal => {
                fill_round_rect(cv.pm, r, 8.0, tokens::surface_control(), cv.ts);
                stroke_round_rect(cv.pm, r, 8.0, tokens::border_subtle(), 1.2, cv.ts);
                tokens::text_secondary()
            }
        };
        let s = t(lang, *label);
        let sw2 = sysfont::measure(s, 1.3);
        let has_hint = !hint.is_empty();
        let ty = if has_hint { r.1 + 6.0 } else { r.1 + (r.3 - 9.1) / 2.0 };
        cv.ui_text_b(s, r.0 + (r.2 - sw2) / 2.0, ty, 1.3, fg);
        if has_hint {
            let hw = sysfont::measure(hint, 1.0);
            cv.ui_text(hint, r.0 + (r.2 - hw) / 2.0, r.1 + r.3 - 11.0, 1.0, tokens::text_muted());
        }
    }

    // CLIP INFO
    let info_y = el.action_y0 + 2.0 * (el.action_h + 6.0) + 12.0;
    cv.ui_text(t(lang, Msg::ClipInfo), px, info_y, 1.1, muted);
    let kb = clip.text.len();
    let size = if kb >= 1024 { format!("{:.1} KB", kb as f32 / 1024.0) } else { format!("{kb} B") };
    let info = [
        (Msg::DetailId, format!("#{:08x}", clip.id)),
        (Msg::DetailCreated, fmt_datetime(clip.ts)),
        (Msg::DetailSource, clip.source.clone().unwrap_or_else(|| "—".into())),
        (Msg::DetailSize, size),
        (Msg::DetailType, "Text".into()),
    ];
    for (i, (label, value)) in info.iter().enumerate() {
        let y = info_y + 16.0 + i as f32 * 15.0;
        cv.ui_text(t(lang, *label), px, y, 1.25, muted);
        let val = sysfont::truncate_to_width(value, 1.25, pw - 64.0);
        let vw = sysfont::measure(&val, 1.25);
        cv.ui_text(&val, px + pw - vw, y, 1.25, txt2);
    }

    // tip box at the bottom
    let tip_h = 40.0;
    let tip_y = dy + dh - tip_h - 12.0;
    fill_round_rect(cv.pm, (px, tip_y, pw, tip_h), 10.0, fade(tokens::ACCENT_GOLD, 0.10), cv.ts);
    cv.ui_text_b("Tip", px + 10.0, tip_y + 7.0, 1.2, tokens::ACCENT_GOLD);
    let tip = t(lang, Msg::ExpTip);
    // wrap the tip across two lines within the box
    let max = pw - 20.0;
    let (l1, l2) = wrap_two(tip, 1.05, max);
    cv.ui_text(&l1, px + 10.0, tip_y + 20.0, 1.05, txt2);
    if !l2.is_empty() {
        cv.ui_text(&l2, px + 10.0, tip_y + 31.0, 1.05, txt2);
    }
}

/// Visual style of a detail action button.
enum ExpKind {
    Primary,
    Normal,
    Muted,
    Danger,
}

/// Split `s` into at most two lines that each fit `max` width (greedy on
/// whitespace; the second line is truncated if it still overflows).
fn wrap_two(s: &str, px: f32, max: f32) -> (String, String) {
    if sysfont::measure(s, px) <= max {
        return (s.to_string(), String::new());
    }
    let mut l1 = String::new();
    let mut rest = String::new();
    let mut filling = true;
    for word in s.split(' ') {
        if filling {
            let trial = if l1.is_empty() { word.to_string() } else { format!("{l1} {word}") };
            if sysfont::measure(&trial, px) <= max {
                l1 = trial;
                continue;
            }
            filling = false;
        }
        if rest.is_empty() {
            rest = word.to_string();
        } else {
            rest.push(' ');
            rest.push_str(word);
        }
    }
    (l1, sysfont::truncate_to_width(&rest, px, max))
}

/// Gold progress ring (track + arc from 12 o'clock) for the level badge.
fn progress_ring(cv: &mut Cv, cx: f32, cy: f32, r: f32, pct: f32) {
    cv.stroke(&oval(cx, cy, r, r), tokens::surface_control(), 3.0);
    let n = 48usize;
    let steps = (pct.clamp(0.0, 1.0) * n as f32).round() as usize;
    if steps == 0 {
        return;
    }
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let a = -std::f32::consts::FRAC_PI_2 + (i as f32 / n as f32) * std::f32::consts::TAU;
        pts.push((cx + a.cos() * r, cy + a.sin() * r));
    }
    cv.line(&pts, tokens::ACCENT_GOLD, 3.0);
}

/// A pill toggle switch (gold when on).
fn toggle_switch(cv: &mut Cv, r: (f32, f32, f32, f32), on: bool) {
    let track = if on { tokens::ACCENT_GOLD } else { tokens::surface_control_active() };
    fill_round_rect(cv.pm, r, r.3 / 2.0, track, cv.ts);
    let knob_r = r.3 / 2.0 - 2.0;
    let kx = if on { r.0 + r.2 - knob_r - 2.0 } else { r.0 + knob_r + 2.0 };
    cv.fill(&oval(kx, r.1 + r.3 / 2.0, knob_r, knob_r), (255, 255, 255, 240));
}

/// A tiny cat face for the sidebar avatar.
fn mini_cat(cv: &mut Cv, cx: f32, cy: f32) {
    // ears
    for s in [-1.0f32, 1.0] {
        let mut pb = PathBuilder::new();
        pb.move_to(cx + s * 13.0, cy - 6.0);
        pb.line_to(cx + s * 16.0, cy - 20.0);
        pb.line_to(cx + s * 3.0, cy - 13.0);
        pb.close();
        let ear = pb.finish();
        cv.fill(&ear, FUR);
        cv.stroke(&ear, OUTLINE, 1.6);
    }
    cv.fill(&oval(cx, cy, 16.0, 14.0), FUR);
    cv.stroke(&oval(cx, cy, 16.0, 14.0), OUTLINE, 1.8);
    cv.fill(&oval(cx - 6.0, cy - 1.0, 1.8, 2.2), OUTLINE);
    cv.fill(&oval(cx + 6.0, cy - 1.0, 1.8, 2.2), OUTLINE);
    cv.fill(&oval(cx - 7.5, cy + 4.0, 3.0, 1.8), fade(BLUSH, 0.7));
    cv.fill(&oval(cx + 7.5, cy + 4.0, 3.0, 1.8), fade(BLUSH, 0.7));
    let mut m = PathBuilder::new();
    m.move_to(cx - 3.0, cy + 4.0);
    m.quad_to(cx, cy + 7.0, cx + 3.0, cy + 4.0);
    cv.stroke(&m.finish(), OUTLINE, 1.5);
}

/// Colored line icon for a sidebar stat card.
fn stat_icon(cv: &mut Cv, which: Msg, cx: f32, cy: f32, color: (u8, u8, u8, u8)) {
    match which {
        Msg::BubbleKeys => {
            cv.stroke(&round_rect(cx - 7.0, cy - 4.0, 14.0, 9.0, 2.0), color, 1.4);
            cv.line(&[(cx - 3.0, cy + 2.0), (cx + 3.0, cy + 2.0)], color, 1.3);
        }
        Msg::BubbleClicks => {
            cv.stroke(&oval(cx, cy, 4.5, 6.0), color, 1.4);
            cv.line(&[(cx, cy - 6.0), (cx, cy - 2.0)], color, 1.3);
        }
        _ => {
            cv.stroke(&round_rect(cx - 5.0, cy - 6.0, 10.0, 13.0, 2.0), color, 1.4);
            cv.fill(&round_rect(cx - 2.5, cy - 8.0, 5.0, 3.0, 1.0), color);
        }
    }
}

/// Line icon for a sidebar nav item.
fn nav_icon(cv: &mut Cv, nav: pl::NavView, cx: f32, cy: f32, color: (u8, u8, u8, u8)) {
    match nav {
        pl::NavView::Clipboard => {
            cv.stroke(&round_rect(cx - 5.0, cy - 6.0, 10.0, 13.0, 2.0), color, 1.4);
            cv.fill(&round_rect(cx - 2.5, cy - 8.0, 5.0, 3.0, 1.0), color);
        }
        pl::NavView::Pinned => {
            let s = star_path(cx, cy, 6.0, 0.0);
            cv.stroke(&s, color, 1.4);
        }
        pl::NavView::Statistics => {
            for (i, h) in [4.0f32, 8.0, 5.5].iter().enumerate() {
                let x = cx - 5.0 + i as f32 * 5.0;
                cv.line(&[(x, cy + 6.0), (x, cy + 6.0 - h)], color, 1.6);
            }
        }
        pl::NavView::Customization => {
            cv.line(&[(cx - 6.0, cy - 3.0), (cx + 6.0, cy - 3.0)], color, 1.4);
            cv.line(&[(cx - 6.0, cy + 3.0), (cx + 6.0, cy + 3.0)], color, 1.4);
            cv.fill(&oval(cx + 2.0, cy - 3.0, 2.2, 2.2), color);
            cv.fill(&oval(cx - 2.0, cy + 3.0, 2.2, 2.2), color);
        }
        pl::NavView::Settings => {
            cv.stroke(&oval(cx, cy, 5.5, 5.5), color, 1.4);
            cv.fill(&oval(cx, cy, 2.0, 2.0), color);
        }
    }
}

fn trash_icon(cv: &mut Cv, cx: f32, cy: f32, color: (u8, u8, u8, u8)) {
    cv.line(&[(cx - 5.0, cy - 4.0), (cx + 5.0, cy - 4.0)], color, 1.5);
    cv.line(&[(cx - 1.8, cy - 5.6), (cx + 1.8, cy - 5.6)], color, 1.5);
    let mut pb = PathBuilder::new();
    pb.move_to(cx - 3.6, cy - 4.0);
    pb.line_to(cx - 3.0, cy + 5.2);
    pb.line_to(cx + 3.0, cy + 5.2);
    pb.line_to(cx + 3.6, cy - 4.0);
    cv.stroke(&pb.finish(), color, 1.5);
}

/// epoch seconds -> "YYYY-MM-DD HH:MM:SS" (UTC), dependency-free.
fn fmt_datetime(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

/// Which list row (if any) is under canvas-y `hy` in the expanded list.
fn row_at_expanded(el: &pl::ExpandedLayout, hy: f32) -> Option<usize> {
    if hy < el.rows_y {
        return None;
    }
    let i = ((hy - el.rows_y) / pl::ROW_H) as usize;
    if i < el.rows { Some(i) } else { None }
}

fn nav_msg(nav: pl::NavView) -> Msg {
    match nav {
        pl::NavView::Clipboard => Msg::NavClipboard,
        pl::NavView::Pinned => Msg::NavPinned,
        pl::NavView::Statistics => Msg::NavStatistics,
        pl::NavView::Customization => Msg::NavCustomization,
        pl::NavView::Settings => Msg::NavSettings,
    }
}

fn nav_count(nav: pl::NavView, store: &ClipStore) -> Option<usize> {
    match nav {
        pl::NavView::Clipboard => Some(store.len()),
        pl::NavView::Pinned => Some(store.pinned_count()),
        _ => None,
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
