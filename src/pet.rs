//! The platform-agnostic pet simulation: animation state machine, particle
//! system, XP/level progression, the copy-event fish, the clipboard store +
//! panel, and scene construction. Knows nothing about windows, hooks or
//! trays — the platform backends drive it by feeding input counts and copy
//! events, calling [`Pet::advance`] each tick and presenting [`Pet::render`].

use crate::clipboard::ClipStore;
use crate::i18n::{self, t, Lang, Msg};
use crate::panel::{self, NavKey, Panel, PanelAction};
use crate::render::{self, Accessory, Badge, BubbleData, FishView, Particle, ParticleKind, Scene};
use crate::sound;
use crate::state::{level_progress, Persist, ACCESSORIES};
use std::collections::VecDeque;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tiny_skia::Pixmap;

/// Selectable pet sizes (small / normal / large) as canvas multipliers.
pub const SCALES: [f32; 3] = [0.78, 1.0, 1.3];

/// XP granted for every captured copy event.
pub const XP_PER_COPY: u64 = 5;

/// Flight time of one fish, in seconds.
const FISH_SECS: f32 = 0.9;
/// At most this many fish queue up during copy bursts.
const FISH_QUEUE_MAX: usize = 3;

/// Logical window size in physical pixels for a given scale (cat only).
pub fn window_size(scale: f32) -> (i32, i32) {
    (
        (render::CANVAS_W * scale).round() as i32,
        (render::CANVAS_H * scale).round() as i32,
    )
}

fn rand_f(seed: &mut u32) -> f32 {
    let mut x = *seed;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *seed = x;
    (x as f32) / (u32::MAX as f32)
}

fn ease_press(p: f32) -> f32 {
    // snappy down, smooth up
    1.0 - (1.0 - p) * (1.0 - p)
}

/// A seed derived from the wall clock; avoids `rand` as a dependency.
fn seed() -> u32 {
    0x1234_5678
        ^ (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(1)
            | 1)
}

pub struct Pet {
    pub st: Persist,
    pub clips: ClipStore,
    pub panel: Panel,
    pub dirty: bool,
    last_save: Instant,
    // timing
    start: Instant,
    last_tick: Instant,
    // animation
    paw_l: f32,
    paw_r: f32,
    next_paw_left: bool,
    blink_start: f32,
    blink_next: f32,
    happy: f32,
    sleep: f32,
    squash: f32,
    rate: f32,
    tail_phase: f32,
    last_event: Instant,
    particles: Vec<Particle>,
    zzz_next: f32,
    toast: Option<(String, f32)>, // text, expires_at (seconds since start)
    bubble_alpha: f32,
    // copy-event fish
    fish: Option<(Badge, f32)>, // badge, flight progress 0..1
    fish_queue: VecDeque<Badge>,
    // inputs from the platform
    hover: bool,
    panel_hint: String,
    // bookkeeping
    frame: u64,
    rng: u32,
    level: u32,
    last_min_bucket: u64,
    level_changed: bool,
    size_changed: bool,
}

impl Pet {
    pub fn new(st: Persist) -> Pet {
        let now = Instant::now();
        let (level, _, _) = level_progress(st.total_xp);
        Pet {
            st,
            clips: ClipStore::load(),
            panel: Panel::default(),
            dirty: false,
            last_save: now,
            start: now,
            last_tick: now,
            paw_l: 0.0,
            paw_r: 0.0,
            next_paw_left: true,
            blink_start: -1.0,
            blink_next: 2.0,
            happy: 0.0,
            sleep: 0.0,
            squash: 0.0,
            rate: 0.0,
            tail_phase: 0.0,
            last_event: now,
            particles: Vec::new(),
            zzz_next: 0.0,
            toast: None,
            bubble_alpha: 0.0,
            fish: None,
            fish_queue: VecDeque::new(),
            hover: false,
            panel_hint: String::new(),
            frame: 0,
            rng: seed(),
            level,
            last_min_bucket: 0,
            level_changed: false,
            size_changed: false,
        }
    }

    pub fn scale(&self) -> f32 {
        SCALES[self.st.scale_idx.min(2)]
    }

    pub fn lang(&self) -> Lang {
        self.st.lang()
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn tooltip(&self) -> String {
        format!("ClipCat — LV {}", self.level)
    }

    pub fn set_hover(&mut self, hover: bool) {
        self.hover = hover;
    }

    /// Footer hint shown in the panel (backend-specific hotkey text).
    pub fn set_panel_hint(&mut self, hint: impl Into<String>) {
        self.panel_hint = hint.into();
    }

    /// Returns `true` once after a level-up so the platform can refresh tray UI.
    pub fn take_level_changed(&mut self) -> bool {
        std::mem::take(&mut self.level_changed)
    }

    /// Returns `true` once after the wanted window size changed (panel
    /// toggled or scale changed); the platform then resizes its surface.
    pub fn take_size_changed(&mut self) -> bool {
        std::mem::take(&mut self.size_changed)
    }

    /// Window size in physical pixels for the current scale + panel state.
    pub fn canvas_size(&self) -> (i32, i32) {
        let s = self.scale();
        if self.panel.open {
            (
                (panel::CANVAS_W * s).round() as i32,
                (panel::CANVAS_H * s).round() as i32,
            )
        } else {
            window_size(s)
        }
    }

    /// Top-left of the cat canvas inside the window canvas.
    fn origin(&self) -> (f32, f32) {
        if self.panel.open {
            panel::CAT_ORIGIN
        } else {
            (0.0, 0.0)
        }
    }

    /// Maps window-canvas coords to cat-local coords (for click_bounce).
    pub fn cat_point(&self, cx: f32, cy: f32) -> (f32, f32) {
        let (ox, oy) = self.origin();
        (cx - ox, cy - oy)
    }

    pub fn set_scale_idx(&mut self, idx: usize) {
        if idx != self.st.scale_idx && idx < SCALES.len() {
            self.st.scale_idx = idx;
            self.dirty = true;
            self.size_changed = true;
        }
    }

    // ---- clipboard ----------------------------------------------------------

    /// A copy was observed system-wide. Stores the clip, feeds the cat a
    /// fish and grants XP. `badge` lets the backend attach a real app icon;
    /// when None one is derived from the source app name.
    pub fn on_copy(&mut self, text: String, source: Option<String>, badge: Option<Badge>) {
        if !self.st.clip_capture {
            return;
        }
        let badge = badge.unwrap_or_else(|| Badge::from_source(source.as_deref()));
        if !self.clips.add_copy(text, source) {
            return;
        }
        if self.fish_queue.len() >= FISH_QUEUE_MAX {
            self.fish_queue.pop_front();
        }
        self.fish_queue.push_back(badge);
        self.st.copies_today += 1;
        self.st.total_copies += 1;
        self.st.total_xp += XP_PER_COPY;
        self.dirty = true;
        self.last_event = Instant::now();
        self.maybe_level_up();
    }

    pub fn panel_open(&self) -> bool {
        self.panel.open
    }

    pub fn toggle_panel(&mut self) {
        self.panel.toggle();
        self.size_changed = true;
    }

    /// Tracks the cursor (window-canvas coords) for panel hover highlights.
    pub fn set_cursor(&mut self, cx: f32, cy: f32) {
        self.panel.cursor = Some((cx, cy));
    }

    pub fn clear_cursor(&mut self) {
        self.panel.cursor = None;
    }

    /// True when the point (window-canvas coords) lands on the open panel.
    pub fn panel_hit(&self, cx: f32, cy: f32) -> bool {
        self.panel.hit(cx, cy)
    }

    /// Executes a panel action. Returns text the backend must put on the
    /// OS clipboard, if any.
    fn run_action(&mut self, action: PanelAction) -> Option<String> {
        match action {
            PanelAction::Copy(id) => {
                let text = self.clips.get(id).map(|c| c.text.clone());
                if text.is_some() {
                    self.set_toast(t(self.lang(), Msg::ToastCopied).to_string(), 1.6);
                    self.happy = (self.happy + 0.4).min(1.0);
                    if self.st.sound_mode >= 1 {
                        sound::play_pop();
                    }
                }
                return text;
            }
            PanelAction::TogglePin(id) => self.clips.toggle_pin(id),
            PanelAction::Delete(id) => {
                self.clips.delete(id);
            }
            PanelAction::Clear => {
                let n = self.clips.clear_unpinned();
                if n > 0 {
                    self.set_toast(i18n::cleared_clips(self.lang(), n), 2.2);
                }
            }
            PanelAction::ToggleCapture => {
                self.st.clip_capture = !self.st.clip_capture;
                self.dirty = true;
                let msg = if self.st.clip_capture {
                    Msg::ToastCaptureOn
                } else {
                    Msg::ToastCapturePaused
                };
                self.set_toast(t(self.lang(), msg).to_string(), 2.2);
            }
            PanelAction::ToggleLang => {
                let lang = self.lang().toggled();
                self.st.set_lang(lang);
                self.dirty = true;
            }
            PanelAction::Close => self.toggle_panel(),
        }
        None
    }

    /// A click at window-canvas coords while the panel is open.
    pub fn panel_click(&mut self, cx: f32, cy: f32) -> Option<String> {
        let action = self.panel.click(cx, cy, &self.clips)?;
        self.run_action(action)
    }

    /// Mouse wheel over the panel (positive rows = scroll down).
    pub fn panel_wheel(&mut self, rows: i32) {
        self.panel.wheel(rows, &self.clips);
    }

    /// Printable character while the panel is open (search input).
    pub fn panel_char(&mut self, c: char) {
        self.panel.input_char(c);
    }

    /// Navigation key while the panel is open.
    pub fn panel_nav(&mut self, key: NavKey) -> Option<String> {
        let action = self.panel.nav(key, &self.clips)?;
        self.run_action(action)
    }

    // ---- per-frame update --------------------------------------------------

    /// Consumes input counts, advances the simulation one frame and returns a
    /// hint of whether the visuals changed enough to be worth presenting.
    pub fn advance(&mut self, k: u64, c: u64, wh: u64) -> bool {
        let now = Instant::now();
        let t = self.now_t();
        let dt = (now - self.last_tick).as_secs_f32().clamp(0.001, 0.1);
        self.last_tick = now;

        if k + c + wh > 0 {
            if self.sleep > 0.5 {
                self.spawn_sparkles(3, 120.0, 100.0); // waking up
            }
            self.last_event = now;
            self.st.total_keys += k;
            self.st.total_clicks += c;
            self.st.keys_today += k;
            self.st.clicks_today += c;
            self.st.total_xp += 2 * k + c + wh;
            self.dirty = true;

            // active-minute tracking
            if let Ok(d) = SystemTime::now().duration_since(UNIX_EPOCH) {
                let bucket = d.as_secs() / 60;
                if bucket != self.last_min_bucket {
                    self.last_min_bucket = bucket;
                    self.st.active_min_today += 1;
                }
            }

            // paw taps (alternate; cap per tick so mashing doesn't teleport)
            let taps = (k + c).min(3);
            for _ in 0..taps {
                if self.next_paw_left {
                    self.paw_l = 1.0;
                } else {
                    self.paw_r = 1.0;
                }
                self.next_paw_left = !self.next_paw_left;
            }
            if taps > 0 && self.st.sound_mode >= 2 {
                sound::play_tap(self.next_paw_left);
            }

            self.maybe_level_up();
        }

        // ease animation params toward rest
        self.paw_l = (self.paw_l - dt * 10.0).max(0.0);
        self.paw_r = (self.paw_r - dt * 10.0).max(0.0);
        self.happy = (self.happy - dt / 1.8).max(0.0);
        self.squash = (self.squash - dt * 6.0).max(0.0);

        let inst_rate = (k + c) as f32 / dt;
        self.rate += (inst_rate - self.rate) * (dt * 2.5).min(1.0);

        let idle_secs = (now - self.last_event).as_secs_f32();
        let sleep_target = if idle_secs > 75.0 { 1.0 } else { 0.0 };
        self.sleep += (sleep_target - self.sleep) * (dt * 1.5).min(1.0);
        self.sleep = self.sleep.clamp(0.0, 1.0);

        if t >= self.blink_next {
            self.blink_start = t;
            self.blink_next = t + 2.2 + rand_f(&mut self.rng) * 3.5;
        }

        let excite = (self.rate / 7.0).clamp(0.0, 1.0);
        self.tail_phase += dt * (1.3 + excite * 5.0 + self.happy * 2.0 - self.sleep * 0.9);

        // fish flight
        if self.fish.is_none() {
            if let Some(badge) = self.fish_queue.pop_front() {
                self.fish = Some((badge, 0.0));
            }
        }
        if let Some((_, ft)) = &mut self.fish {
            *ft += dt / FISH_SECS;
            if *ft >= 1.0 {
                self.fish = None;
                self.nom();
            }
        }

        // zzz particles while asleep
        if self.sleep > 0.7 && t > self.zzz_next {
            self.zzz_next = t + 1.4;
            let r = rand_f(&mut self.rng);
            self.particles.push(Particle {
                x: 150.0 + r * 10.0,
                y: 92.0,
                vx: 7.0 + r * 6.0,
                vy: -15.0,
                life: 1.0,
                kind: ParticleKind::Zzz,
                size: 2.0,
                spin: 0.0,
            });
        }

        // particle physics
        for p in self.particles.iter_mut() {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            match p.kind {
                ParticleKind::Star => {
                    p.vy += 110.0 * dt;
                    p.spin += dt * 4.0;
                    p.life -= dt / 1.1;
                }
                ParticleKind::Heart => {
                    p.vy += (-26.0 - p.vy) * dt * 2.0;
                    p.life -= dt / 1.3;
                }
                ParticleKind::Sparkle => p.life -= dt / 0.7,
                ParticleKind::Zzz => p.life -= dt / 2.6,
            }
        }
        self.particles.retain(|p| p.life > 0.0);

        // bubble fade (hidden while the panel is open)
        let bubble_target = if !self.panel.open && (self.st.bubble_pinned || self.hover) {
            1.0
        } else {
            0.0
        };
        self.bubble_alpha += (bubble_target - self.bubble_alpha) * (dt * 9.0).min(1.0);

        // expire toast
        if let Some((_, until)) = &self.toast {
            if *until <= t {
                self.toast = None;
            }
        }

        // day rollover (platform drives the throttled autosave so it can
        // capture the live window position first — see `should_autosave`)
        if self.st.roll_day() {
            self.dirty = true;
        }

        // redraw hint: skip every other frame only when fully asleep & still
        self.frame += 1;
        let resting = self.sleep > 0.9
            && self.particles.is_empty()
            && self.fish.is_none()
            && !self.panel.open;
        !(resting && self.frame.is_multiple_of(2))
    }

    /// The fish reached the mouth: crunch, sparkles, a happy bump.
    fn nom(&mut self) {
        self.happy = (self.happy + 0.5).min(1.0);
        self.squash = 0.6;
        self.spawn_sparkles(4, 120.0, 130.0);
        let r = rand_f(&mut self.rng);
        self.particles.push(Particle {
            x: 120.0,
            y: 118.0,
            vx: (r - 0.5) * 18.0,
            vy: -32.0,
            life: 1.0,
            kind: ParticleKind::Heart,
            size: 4.0,
            spin: 0.0,
        });
        if self.st.sound_mode >= 1 {
            sound::play_nom();
        }
    }

    /// Fish position along its arc into the mouth (cat-local coords).
    fn fish_view<'a>(badge: &'a Badge, ft: f32) -> FishView<'a> {
        // quadratic bezier: top-right, dipping in toward the mouth
        let (sx, sy) = (236.0f32, 44.0f32);
        let (cx, cy) = (186.0f32, -6.0f32);
        let (mx, my) = (122.0f32, 134.0f32);
        let u = 1.0 - ft;
        let x = u * u * sx + 2.0 * u * ft * cx + ft * ft * mx;
        let y = u * u * sy + 2.0 * u * ft * cy + ft * ft * my;
        // velocity for heading
        let dx = 2.0 * u * (cx - sx) + 2.0 * ft * (mx - cx);
        let dy = 2.0 * u * (cy - sy) + 2.0 * ft * (my - cy);
        let mut rot = dy.atan2(dx).to_degrees() - 180.0;
        rot += (ft * 14.0).sin() * 6.0; // swimming wiggle
        // gulp: shrink as it disappears into the mouth
        let scale = if ft > 0.82 {
            (1.0 - (ft - 0.82) / 0.18 * 0.85).max(0.15)
        } else {
            1.0
        } * 0.95;
        FishView {
            x,
            y,
            rot,
            scale,
            badge,
        }
    }

    /// Draws onto a transparent canvas (native layered-window backend).
    pub fn render(&self, pm: &mut Pixmap) {
        self.draw(pm, false);
    }

    /// Draws onto an opaque card background (portable backend).
    pub fn render_card(&self, pm: &mut Pixmap) {
        self.draw(pm, true);
    }

    /// Builds the scene and rasterizes it with the chosen background.
    fn draw(&self, pm: &mut Pixmap, card: bool) {
        let t = self.now_t();
        let bt = t - self.blink_start;
        let blink = if bt < 0.14 {
            1.0 - ((bt / 0.07) - 1.0).abs()
        } else {
            0.0
        };
        let excite = (self.rate / 7.0).clamp(0.0, 1.0);
        let breath = (t * (2.2 - self.sleep * 1.2)).sin();

        let toast_view = self
            .toast
            .as_ref()
            .map(|(s, until)| (s.as_str(), ((*until - t) / 0.4).min(1.0)));

        let (lv, into, need) = level_progress(self.st.total_xp);
        let bubble = if self.bubble_alpha > 0.01 {
            Some(BubbleData {
                level: lv,
                pct: into as f32 / need as f32,
                keys: self.st.keys_today,
                clicks: self.st.clicks_today,
                copies: self.st.copies_today,
                minutes: self.st.active_min_today,
            })
        } else {
            None
        };

        let fish = self.fish.as_ref().map(|(b, ft)| Self::fish_view(b, *ft));
        // mouth opens as the fish closes in
        let mouth_open = match &self.fish {
            Some((_, ft)) => ((ft - 0.45) / 0.4).clamp(0.0, 1.0),
            None => 0.0,
        };

        let scene = Scene {
            paw_l: ease_press(self.paw_l),
            paw_r: ease_press(self.paw_r),
            blink,
            happy: self.happy,
            sleep: self.sleep,
            excite,
            squash: self.squash,
            breath,
            tail_phase: self.tail_phase,
            mouth_open,
            accessory: Accessory::from_id(self.st.accessory),
            particles: &self.particles,
            fish,
            bubble,
            bubble_alpha: self.bubble_alpha,
            toast: toast_view,
            lang: self.lang(),
            origin: self.origin(),
        };
        if card {
            render::render_card(pm, &scene, self.scale());
        } else {
            render::render(pm, &scene, self.scale());
        }
        if self.panel.open {
            let view = render::PanelView {
                panel: &self.panel,
                store: &self.clips,
                lang: self.lang(),
                capture: self.st.clip_capture,
                hint: &self.panel_hint,
                caret: (t * 1.6).fract() < 0.65,
            };
            render::draw_panel(pm, &view, self.scale());
        }
    }

    // ---- interactions ------------------------------------------------------

    /// Double-click / explicit pet: a burst of hearts and bonus XP.
    pub fn pet(&mut self) {
        self.happy = 1.0;
        self.st.total_xp += 10;
        self.dirty = true;
        for _ in 0..6 {
            let r1 = rand_f(&mut self.rng);
            let r2 = rand_f(&mut self.rng);
            self.particles.push(Particle {
                x: 95.0 + r1 * 50.0,
                y: 92.0 + r2 * 16.0,
                vx: (r1 - 0.5) * 30.0,
                vy: -35.0 - r2 * 25.0,
                life: 1.0,
                kind: ParticleKind::Heart,
                size: 4.5 + r1 * 3.0,
                spin: 0.0,
            });
        }
        if self.st.sound_mode >= 1 {
            sound::play_pop();
        }
        self.maybe_level_up();
    }

    /// Single tap on the body: a little squash bounce and a sparkle.
    /// `cx`/`cy` are in cat-local canvas coordinates (see [`Pet::cat_point`]).
    pub fn click_bounce(&mut self, cx: f32, cy: f32) {
        self.squash = 1.0;
        self.st.total_xp += 1;
        self.dirty = true;
        let r = rand_f(&mut self.rng);
        self.particles.push(Particle {
            x: cx,
            y: cy - 6.0,
            vx: (r - 0.5) * 14.0,
            vy: -28.0,
            life: 1.0,
            kind: ParticleKind::Sparkle,
            size: 5.0,
            spin: 0.0,
        });
    }

    pub fn reset_stats(&mut self) {
        self.st.total_keys = 0;
        self.st.total_clicks = 0;
        self.st.total_copies = 0;
        self.st.total_xp = 0;
        self.st.keys_today = 0;
        self.st.clicks_today = 0;
        self.st.copies_today = 0;
        self.st.active_min_today = 0;
        self.st.accessory = 0;
        self.level = 1;
        self.level_changed = true;
        self.save();
    }

    fn maybe_level_up(&mut self) {
        let (lv, _, _) = level_progress(self.st.total_xp);
        if lv > self.level {
            let t = self.now_t();
            self.level_up(lv, t);
        }
    }

    fn level_up(&mut self, lv: u32, t: f32) {
        self.level = lv;
        self.level_changed = true;
        self.happy = 1.0;
        self.spawn_stars(12);
        let mut text = i18n::level_up(self.lang(), lv);
        for (i, acc) in ACCESSORIES.iter().enumerate() {
            if acc.level == lv {
                self.st.accessory = i + 1;
                text = i18n::new_accessory(self.lang(), acc.name(self.lang()));
                self.spawn_sparkles(8, 120.0, 80.0);
            }
        }
        self.toast = Some((text, t + 3.2));
        if self.st.sound_mode >= 1 {
            sound::play_chime();
        }
        self.dirty = true;
    }

    fn set_toast(&mut self, text: String, secs: f32) {
        let t = self.now_t();
        self.toast = Some((text, t + secs));
    }

    fn spawn_stars(&mut self, n: usize) {
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let r = rand_f(&mut self.rng);
            let v = 65.0 + r * 45.0;
            self.particles.push(Particle {
                x: 120.0,
                y: 120.0,
                vx: a.cos() * v,
                vy: a.sin() * v - 35.0,
                life: 1.0,
                kind: ParticleKind::Star,
                size: 5.0 + r * 3.5,
                spin: r * 6.0,
            });
        }
    }

    fn spawn_sparkles(&mut self, n: usize, x: f32, y: f32) {
        for _ in 0..n {
            let r1 = rand_f(&mut self.rng);
            let r2 = rand_f(&mut self.rng);
            self.particles.push(Particle {
                x: x + (r1 - 0.5) * 60.0,
                y: y + (r2 - 0.5) * 40.0,
                vx: (r1 - 0.5) * 20.0,
                vy: -20.0 - r2 * 15.0,
                life: 1.0,
                kind: ParticleKind::Sparkle,
                size: 4.0 + r1 * 3.0,
                spin: 0.0,
            });
        }
    }

    // ---- persistence -------------------------------------------------------

    fn now_t(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    /// True when there are unsaved changes and enough time has elapsed that
    /// the platform should capture the window position and persist.
    pub fn should_autosave(&self) -> bool {
        (self.dirty || self.clips.dirty) && self.last_save.elapsed().as_secs() >= 30
    }

    /// Records the current window position and writes state to disk.
    pub fn save_pos(&mut self, x: i32, y: i32) {
        self.st.pos_x = x;
        self.st.pos_y = y;
        self.st.has_pos = true;
        self.save();
    }

    pub fn save(&mut self) {
        self.st.save();
        self.clips.save_if_dirty();
        self.dirty = false;
        self.last_save = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::PanelAction;

    fn pet() -> Pet {
        let mut p = Pet::new(Persist::default());
        p.clips = ClipStore::default(); // don't touch the real config dir
        p
    }

    #[test]
    fn copy_event_stores_clip_grants_xp_and_queues_fish() {
        let mut p = pet();
        let xp0 = p.st.total_xp;
        p.on_copy("hello".into(), Some("Code".into()), None);
        assert_eq!(p.clips.len(), 1);
        assert_eq!(p.st.total_xp, xp0 + XP_PER_COPY);
        assert_eq!(p.st.copies_today, 1);
        assert_eq!(p.fish_queue.len(), 1);
        assert_eq!(p.fish_queue[0].letter, 'C');
    }

    #[test]
    fn capture_pause_ignores_copies() {
        let mut p = pet();
        p.st.clip_capture = false;
        p.on_copy("secret".into(), None, None);
        assert!(p.clips.is_empty());
        assert!(p.fish_queue.is_empty());
        assert_eq!(p.st.total_xp, 0);
    }

    #[test]
    fn fish_queue_is_capped() {
        let mut p = pet();
        for i in 0..10 {
            p.on_copy(format!("clip {i}"), None, None);
        }
        assert!(p.fish_queue.len() <= FISH_QUEUE_MAX);
    }

    #[test]
    fn fish_flies_and_gets_eaten() {
        let mut p = pet();
        p.on_copy("fish food".into(), None, None);
        // simulate ~1.5s of ticks
        for _ in 0..50 {
            p.last_tick -= std::time::Duration::from_millis(33);
            p.advance(0, 0, 0);
        }
        assert!(p.fish.is_none());
        assert!(p.fish_queue.is_empty());
        assert!(p.happy > 0.0, "nom should make the cat happy");
    }

    #[test]
    fn panel_copy_returns_text_for_backend() {
        let mut p = pet();
        p.on_copy("copy me back".into(), None, None);
        p.toggle_panel();
        assert!(p.panel_open());
        let got = p.panel_nav(NavKey::Enter);
        assert_eq!(got.as_deref(), Some("copy me back"));
    }

    #[test]
    fn toggle_panel_changes_canvas_size() {
        let mut p = pet();
        let closed = p.canvas_size();
        p.toggle_panel();
        assert!(p.take_size_changed());
        let open = p.canvas_size();
        assert!(open.0 > closed.0 && open.1 > closed.1);
        // cat-local mapping accounts for the origin shift
        let (cx, cy) = p.cat_point(crate::panel::CAT_ORIGIN.0, crate::panel::CAT_ORIGIN.1);
        assert_eq!((cx, cy), (0.0, 0.0));
    }

    #[test]
    fn lang_toggle_persists_in_state() {
        let mut p = pet();
        let before = p.lang();
        p.toggle_panel();
        let _ = p.run_action(PanelAction::ToggleLang);
        assert_ne!(p.lang(), before);
    }

    #[test]
    fn render_panel_open_smoke() {
        let mut p = pet();
        p.on_copy("첫 번째 클립".into(), Some("브라우저".into()), None);
        p.on_copy("second clip".into(), Some("Code".into()), None);
        p.toggle_panel();
        let (w, h) = p.canvas_size();
        let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
        p.render(&mut pm); // must not panic
        p.render_card(&mut pm);
        let drawn = pm.data().chunks_exact(4).any(|px| px[3] > 0);
        assert!(drawn);
    }
}
