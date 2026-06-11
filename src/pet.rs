//! The platform-agnostic pet simulation: animation state machine, particle
//! system, XP/level progression and scene construction. Knows nothing about
//! windows, hooks or trays — the platform backends drive it by feeding input
//! counts, calling [`Pet::advance`] each tick and presenting [`Pet::render`].

use crate::render::{self, Accessory, BubbleData, Particle, ParticleKind, Scene};
use crate::sound;
use crate::state::{level_progress, Persist, ACCESSORIES};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tiny_skia::Pixmap;

/// Selectable pet sizes (small / normal / large) as canvas multipliers.
pub const SCALES: [f32; 3] = [0.78, 1.0, 1.3];

/// Logical window size in physical pixels for a given scale.
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
    // inputs from the platform
    hover: bool,
    // bookkeeping
    frame: u64,
    rng: u32,
    level: u32,
    last_min_bucket: u64,
    level_changed: bool,
}

impl Pet {
    pub fn new(st: Persist) -> Pet {
        let now = Instant::now();
        let (level, _, _) = level_progress(st.total_xp);
        Pet {
            st,
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
            hover: false,
            frame: 0,
            rng: seed(),
            level,
            last_min_bucket: 0,
            level_changed: false,
        }
    }

    pub fn scale(&self) -> f32 {
        SCALES[self.st.scale_idx.min(2)]
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn tooltip(&self) -> String {
        format!("DeskCat — LV {}", self.level)
    }

    pub fn set_hover(&mut self, hover: bool) {
        self.hover = hover;
    }

    /// Returns `true` once after a level-up so the platform can refresh tray UI.
    pub fn take_level_changed(&mut self) -> bool {
        std::mem::take(&mut self.level_changed)
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

            let (lv, _, _) = level_progress(self.st.total_xp);
            if lv > self.level {
                self.level_up(lv, t);
            }
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

        // bubble fade
        let bubble_target = if self.st.bubble_pinned || self.hover {
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
        let resting = self.sleep > 0.9 && self.particles.is_empty();
        !(resting && self.frame.is_multiple_of(2))
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
                minutes: self.st.active_min_today,
            })
        } else {
            None
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
            accessory: Accessory::from_id(self.st.accessory),
            particles: &self.particles,
            bubble,
            bubble_alpha: self.bubble_alpha,
            toast: toast_view,
        };
        if card {
            render::render_card(pm, &scene, self.scale());
        } else {
            render::render(pm, &scene, self.scale());
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
        let (lv, _, _) = level_progress(self.st.total_xp);
        if lv > self.level {
            let t = self.now_t();
            self.level_up(lv, t);
        }
    }

    /// Single tap on the body: a little squash bounce and a sparkle.
    /// `cx`/`cy` are in canvas coordinates (window pixels / scale).
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
        self.st.total_xp = 0;
        self.st.keys_today = 0;
        self.st.clicks_today = 0;
        self.st.active_min_today = 0;
        self.st.accessory = 0;
        self.level = 1;
        self.level_changed = true;
        self.save();
    }

    fn level_up(&mut self, lv: u32, t: f32) {
        self.level = lv;
        self.level_changed = true;
        self.happy = 1.0;
        self.spawn_stars(12);
        let mut text = format!("LEVEL UP! LV {}", lv);
        for (i, acc) in ACCESSORIES.iter().enumerate() {
            if acc.level == lv {
                self.st.accessory = i + 1;
                text = format!("* NEW: {} *", acc.name_en);
                self.spawn_sparkles(8, 120.0, 80.0);
            }
        }
        self.toast = Some((text, t + 3.2));
        if self.st.sound_mode >= 1 {
            sound::play_chime();
        }
        self.dirty = true;
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
        self.dirty && self.last_save.elapsed().as_secs() >= 30
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
        self.dirty = false;
        self.last_save = Instant::now();
    }
}
