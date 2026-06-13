//! The platform-agnostic pet simulation: animation state machine, particle
//! system, XP/level progression, the copy-event fish, the clipboard store +
//! panel, and scene construction. Knows nothing about windows, hooks or
//! trays — the platform backends drive it by feeding input counts and copy
//! events, calling [`Pet::advance`] each tick and presenting [`Pet::render`].

use crate::clipboard::ClipStore;
use crate::i18n::{self, t, Lang, Msg};
use crate::menu::{MenuAction, MenuEntry, MenuItem, MenuOutcome};
use crate::panel::{ExpAction, ExpandedHit, NavKey, Panel, PanelAction, PanelDrag};
use crate::render::{
    self, Accessory, Badge, BubbleData, FishView, Particle, ParticleKind, Scene, XpPop,
};
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

// ---- mood timing (docs/design/docs/03_pet_behavior_spec.md +
//      docs/design/motion/pet_motion_spec.json) -------------------------------
/// Idle seconds before the cat turns Curious, and before it falls asleep.
const CURIOUS_AFTER: f32 = 30.0;
const SLEEP_AFTER: f32 = 75.0;
/// Post-event mood windows (seconds): nom crunch → happy tail; petting; levelup.
const NOM_SECS: f32 = 0.28;
const HAPPY_SECS: f32 = 0.60;
const PETTING_SECS: f32 = 1.1;
const LEVELUP_SECS: f32 = 1.4;
/// Fish flight fraction at which the mouth opens to catch it.
const MOUTH_OPEN_AT: f32 = 0.72;
/// `sleep` scalar above which the cat reads as asleep.
const SLEEP_MOOD_AT: f32 = 0.6;
/// keys/sec thresholds for the typing mood tiers (keys only; auto-repeat is
/// already collapsed upstream — we count, never read, key events).
const KPS_SLOW: f32 = 1.0;
const KPS_FAST: f32 = 5.0;
const KPS_EXTREME: f32 = 10.0;
/// A `pet()` within this many seconds of a `click_bounce` is the same
/// double-click: refund the bounce's +1 XP so a double-click nets exactly +10,
/// not +11/+12 (pet behavior spec, "Petting/boop").
const DBLCLICK_REFUND: f32 = 0.35;

/// Discrete pet mood — the design package's `PetMood` state machine
/// (`docs/design/docs/03_pet_behavior_spec.md`). It is *derived* each frame
/// from the continuous animation scalars and event timers by [`Pet::mood`];
/// rendering still reads the scalars directly, so this is a queryable, testable
/// view of "what the cat is doing", not a second source of animation truth.
/// `Boop` from a single click is folded into the squash bounce within
/// `Idle`/typing; `Happy` is the short tail after `Nom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetMood {
    Idle,
    Curious,
    TypingSlow,
    TypingFast,
    TypingExtreme,
    CopyIncoming,
    MouthOpen,
    Nom,
    Happy,
    Petting,
    LevelUp,
    Sleeping,
    PanelOpen,
}

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
    /// Keys/sec over a ~1s window — drives the typing mood tiers only.
    kps: f32,
    tail_phase: f32,
    last_event: Instant,
    // mood event timers (seconds on the `now_t` clock)
    nom_until: f32,
    happy_until: f32,
    petting_until: f32,
    levelup_until: f32,
    /// `(time, xp)` of the last single-click bounce, for double-click refund.
    last_bounce: Option<(f32, u64)>,
    particles: Vec<Particle>,
    xp_pops: Vec<XpPop>,
    zzz_next: f32,
    /// Next time a typing-extreme spark may emit (motion spec `sparks: true`).
    spark_next: f32,
    toast: Option<(String, f32)>, // text, expires_at (seconds since start)
    bubble_alpha: f32,
    // copy-event fish
    fish: Option<(Badge, f32)>, // badge, flight progress 0..1
    fish_queue: VecDeque<Badge>,
    // inputs from the platform
    hover: bool,
    panel_hint: String,
    update_available: Option<String>, // newer release found (crate::update)
    accessibility_hinted: bool,       // macOS Accessibility hint shown once
    // bookkeeping
    frame: u64,
    rng: u32,
    level: u32,
    last_min_bucket: u64,
    level_changed: bool,
    size_changed: bool,
    /// Window-position delta (physical px) accumulated by layout changes so
    /// the cat stays put on screen; drained via [`Pet::take_window_shift`].
    pending_shift: (f32, f32),
    /// Active panel-card drag (header move / grip resize), if any.
    drag: Option<PanelDrag>,
}

impl Pet {
    pub fn new(st: Persist) -> Pet {
        let now = Instant::now();
        let (level, _, _) = level_progress(st.total_xp);
        let panel = Panel::with_geometry(st.panel_w, st.panel_h, (st.panel_off_x, st.panel_off_y));
        let clips = ClipStore::load();
        let mut pet = Pet {
            st,
            clips,
            panel,
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
            kps: 0.0,
            tail_phase: 0.0,
            last_event: now,
            nom_until: 0.0,
            happy_until: 0.0,
            petting_until: 0.0,
            levelup_until: 0.0,
            last_bounce: None,
            particles: Vec::new(),
            xp_pops: Vec::new(),
            zzz_next: 0.0,
            spark_next: 0.0,
            toast: None,
            bubble_alpha: 0.0,
            fish: None,
            fish_queue: VecDeque::new(),
            hover: false,
            panel_hint: String::new(),
            update_available: None,
            accessibility_hinted: false,
            frame: 0,
            rng: seed(),
            level,
            last_min_bucket: 0,
            level_changed: false,
            size_changed: false,
            pending_shift: (0.0, 0.0),
            drag: None,
        };
        // A corrupt history was backed up and reset during load — say so once.
        if pet.clips.take_recovered_corrupt() {
            pet.set_toast(t(pet.lang(), Msg::ToastClipsCorrupt).to_string(), 5.0);
        }
        pet
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
        format!("ClipCat — {} {}", t(self.lang(), Msg::LevelShort), self.level)
    }

    /// The cat's current discrete mood, derived from the animation scalars and
    /// event timers. Priority follows the motion spec (levelup > copy fish >
    /// petting > typing > panel > sleeping > curious > idle). Queryable view
    /// for the visual states and for tests; it never *drives* animation.
    pub fn mood(&self) -> PetMood {
        let t = self.now_t();
        if t < self.levelup_until {
            return PetMood::LevelUp;
        }
        if let Some((_, ft)) = &self.fish {
            return if *ft >= MOUTH_OPEN_AT {
                PetMood::MouthOpen
            } else {
                PetMood::CopyIncoming
            };
        }
        if t < self.nom_until {
            return PetMood::Nom;
        }
        if t < self.happy_until {
            return PetMood::Happy;
        }
        if t < self.petting_until {
            return PetMood::Petting;
        }
        if self.kps >= KPS_EXTREME {
            return PetMood::TypingExtreme;
        }
        if self.kps >= KPS_FAST {
            return PetMood::TypingFast;
        }
        if self.kps >= KPS_SLOW {
            return PetMood::TypingSlow;
        }
        if self.panel.open {
            return PetMood::PanelOpen;
        }
        if self.sleep > SLEEP_MOOD_AT {
            return PetMood::Sleeping;
        }
        if (Instant::now() - self.last_event).as_secs_f32() > CURIOUS_AFTER {
            return PetMood::Curious;
        }
        PetMood::Idle
    }

    pub fn set_hover(&mut self, hover: bool) {
        self.hover = hover;
    }

    /// Footer hint shown in the panel (backend-specific hotkey text).
    pub fn set_panel_hint(&mut self, hint: impl Into<String>) {
        self.panel_hint = hint.into();
    }

    // ---- auto-update (crate::update, ADR-0009) -------------------------------

    /// The background checker found a newer release: remember it for the
    /// menus/shortcuts and toast it, once per version.
    pub fn notify_update(&mut self, version: &str) {
        if self.update_available.as_deref() == Some(version) {
            return;
        }
        self.update_available = Some(version.to_string());
        self.set_toast(i18n::update_available(self.lang(), version), 4.0);
    }

    /// Newer release version the user can update to, if one was found.
    pub fn update_available(&self) -> Option<&str> {
        self.update_available.as_deref()
    }

    /// The update download/install started (long toast; replaced by the
    /// restart or by [`Pet::notify_update_failed`]).
    pub fn notify_update_downloading(&mut self) {
        self.set_toast(t(self.lang(), Msg::ToastUpdateDownloading).to_string(), 120.0);
    }

    /// The update download/install failed; the menu entry stays for a retry.
    pub fn notify_update_failed(&mut self) {
        self.set_toast(t(self.lang(), Msg::ToastUpdateFailed).to_string(), 3.0);
    }

    /// The macOS global-input event tap could not be installed — Accessibility
    /// permission has not been granted. Point the user at the setting, once;
    /// the pet, clipboard history and panel (C / middle-click) keep working,
    /// only the global hotkey and the keyboard/mouse "tap along" do not.
    pub fn notify_accessibility_needed(&mut self) {
        if self.accessibility_hinted {
            return;
        }
        self.accessibility_hinted = true;
        self.set_toast(t(self.lang(), Msg::ToastAccessibility).to_string(), 8.0);
    }

    /// Returns `true` once after a level-up so the platform can refresh tray UI.
    pub fn take_level_changed(&mut self) -> bool {
        std::mem::take(&mut self.level_changed)
    }

    /// Returns `true` once after the wanted window size changed (panel
    /// toggled/moved/resized or scale changed); the platform then resizes
    /// its surface and offsets the window by [`Pet::take_window_shift`].
    pub fn take_size_changed(&mut self) -> bool {
        std::mem::take(&mut self.size_changed)
    }

    /// Pixel delta to add to the window position alongside a size change so
    /// the cat stays put on screen: opening, moving or resizing the panel
    /// re-origins the canvas around the cat, and a scale change keeps the
    /// cat's feet on the same line. Drained on read; sub-pixel remainders
    /// carry over so slow drags don't drift.
    pub fn take_window_shift(&mut self) -> (i32, i32) {
        let dx = self.pending_shift.0.round();
        let dy = self.pending_shift.1.round();
        self.pending_shift.0 -= dx;
        self.pending_shift.1 -= dy;
        (dx as i32, dy as i32)
    }

    /// Window size in physical pixels for the current scale + panel state.
    pub fn canvas_size(&self) -> (i32, i32) {
        let s = self.scale();
        if self.panel.open {
            let l = self.panel.layout();
            (
                (l.canvas_w * s).round() as i32,
                (l.canvas_h * s).round() as i32,
            )
        } else {
            window_size(s)
        }
    }

    /// Top-left of the cat canvas inside the window canvas.
    fn origin(&self) -> (f32, f32) {
        if self.panel.open {
            self.panel.layout().cat
        } else {
            (0.0, 0.0)
        }
    }

    /// The cat's bottom-center in physical pixels — the screen point every
    /// layout change keeps fixed (see [`Pet::take_window_shift`]).
    fn cat_anchor(&self) -> (f32, f32) {
        let (ox, oy) = self.origin();
        let s = self.scale();
        ((ox + render::CANVAS_W / 2.0) * s, (oy + render::CANVAS_H) * s)
    }

    /// Runs a canvas-layout mutation, accumulating the window shift that
    /// keeps the cat anchored and flagging the size change for the backend.
    fn relayout(&mut self, f: impl FnOnce(&mut Pet)) {
        let before = self.cat_anchor();
        f(self);
        let after = self.cat_anchor();
        self.pending_shift.0 += before.0 - after.0;
        self.pending_shift.1 += before.1 - after.1;
        self.size_changed = true;
    }

    /// Maps window-canvas coords to cat-local coords (for click_bounce).
    pub fn cat_point(&self, cx: f32, cy: f32) -> (f32, f32) {
        let (ox, oy) = self.origin();
        (cx - ox, cy - oy)
    }

    pub fn set_scale_idx(&mut self, idx: usize) {
        if idx != self.st.scale_idx && idx < SCALES.len() {
            self.relayout(|p| p.st.scale_idx = idx);
            self.dirty = true;
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
        // Queue overflow merges into the latest fish (motion spec
        // queue_overflow: merge_latest_with_count): keep the newest source but
        // carry a running +N count instead of dropping older copies.
        if self.fish_queue.len() >= FISH_QUEUE_MAX {
            if let Some(last) = self.fish_queue.back_mut() {
                let mut badge = badge;
                badge.count = last.count + 1;
                *last = badge;
            }
        } else {
            self.fish_queue.push_back(badge);
        }
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

    /// Whether the expanded three-pane screen is showing.
    pub fn panel_expanded(&self) -> bool {
        self.panel.expanded
    }

    pub fn toggle_panel(&mut self) {
        self.drag = None;
        self.relayout(|p| p.panel.toggle());
    }

    // ---- panel card drag (move / resize) -------------------------------------

    /// Begins a card move/resize drag when (cx, cy) — window-canvas coords —
    /// hits the card's header strip or its resize grip. Returns true when a
    /// drag started (the backend then feeds deltas and must not treat the
    /// press as a click).
    pub fn panel_drag_start(&mut self, cx: f32, cy: f32) -> bool {
        self.drag = self.panel.drag_hit(cx, cy);
        self.drag.is_some()
    }

    pub fn panel_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Continues the active drag by a cursor delta in canvas units (screen
    /// pixels / scale). The new geometry is persisted and the backend is
    /// flagged to re-apply size + window shift.
    pub fn panel_drag_update(&mut self, dx: f32, dy: f32) {
        let Some(kind) = self.drag else { return };
        self.relayout(|p| p.panel.drag_by(kind, dx, dy));
        // a shorter card may need the scroll re-clamped
        self.panel.refresh(&self.clips);
        self.st.panel_w = self.panel.w;
        self.st.panel_h = self.panel.h;
        self.st.panel_off_x = self.panel.off.0;
        self.st.panel_off_y = self.panel.off.1;
        self.dirty = true;
    }

    pub fn panel_drag_end(&mut self) {
        self.drag = None;
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

    /// Copies a clip's text to hand back to the backend (toast + bump, no
    /// panel close). Shared by the compact `Copy` action and the expanded
    /// detail/list copy paths.
    fn copy_text(&mut self, id: u64) -> Option<String> {
        let text = self.clips.get(id).map(|c| c.text.clone());
        if text.is_some() {
            self.set_toast(t(self.lang(), Msg::ToastCopied).to_string(), 1.6);
            self.happy = (self.happy + 0.4).min(1.0);
            if self.st.sound_mode >= 1 {
                sound::play_pop();
            }
        }
        text
    }

    /// Executes a panel action. Returns text the backend must put on the
    /// OS clipboard, if any.
    fn run_action(&mut self, action: PanelAction) -> Option<String> {
        match action {
            PanelAction::Copy(id) => {
                let text = self.copy_text(id);
                // picked a clip: close the panel so the user can paste
                // (switchable off to grab several clips in a row). The expanded
                // screen never auto-closes — its detail pane is the point.
                if text.is_some() && self.st.panel_autoclose && !self.panel.expanded {
                    self.toggle_panel();
                }
                return text;
            }
            PanelAction::TogglePin(id) => {
                self.clips.toggle_pin(id);
                // pinning re-orders the list; keep the selection on the clip
                self.panel.focus_id(&self.clips, id);
            }
            PanelAction::Delete(id) => {
                if self.clips.delete(id) {
                    self.panel.refresh(&self.clips);
                    self.set_toast(t(self.lang(), Msg::ToastDeleted).to_string(), 2.2);
                }
            }
            PanelAction::ArmClear => {
                self.set_toast(t(self.lang(), Msg::ToastClearConfirm).to_string(), 2.6);
            }
            PanelAction::Clear => {
                let n = self.clips.clear_unpinned();
                if n > 0 {
                    self.panel.refresh(&self.clips);
                    self.set_toast(i18n::cleared_clips(self.lang(), n), 2.2);
                }
            }
            PanelAction::Undo => {
                if let Some(id) = self.clips.undo_delete() {
                    self.panel.focus_id(&self.clips, id);
                    self.set_toast(t(self.lang(), Msg::ToastRestored).to_string(), 1.8);
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

    /// Flips "close the panel after copying a clip" and confirms via toast.
    pub fn toggle_panel_autoclose(&mut self) {
        self.st.panel_autoclose = !self.st.panel_autoclose;
        self.dirty = true;
        let msg = if self.st.panel_autoclose {
            Msg::ToastAutoCloseOn
        } else {
            Msg::ToastAutoCloseOff
        };
        self.set_toast(t(self.lang(), msg).to_string(), 2.2);
    }

    /// Enter the expanded three-pane screen (opening the panel if needed) or,
    /// if already expanded, collapse back to the compact panel.
    pub fn toggle_expanded(&mut self) {
        self.drag = None;
        self.relayout(|p| {
            if !p.panel.open {
                p.panel.toggle();
            }
            p.panel.toggle_expanded();
        });
    }

    /// A click at window-canvas coords while the panel is open.
    pub fn panel_click(&mut self, cx: f32, cy: f32) -> Option<String> {
        if self.panel.expanded {
            return self.expanded_click(cx, cy);
        }
        let action = self.panel.click(cx, cy, &self.clips)?;
        self.run_action(action)
    }

    /// Routes a click in the expanded screen: collapse, sidebar/toolbar
    /// controls, switch nav, select a row, or run a detail-pane action.
    fn expanded_click(&mut self, cx: f32, cy: f32) -> Option<String> {
        let hit = self.panel.expanded_hit(cx, cy, &self.clips);
        // any interaction except a second clear press disarms the clear button
        if !matches!(hit, ExpandedHit::ClearUnpinned) {
            self.panel.clear_armed = false;
        }
        match hit {
            ExpandedHit::Collapse => {
                self.relayout(|p| p.panel.toggle_expanded());
                None
            }
            ExpandedHit::ToggleAutoclose => {
                self.toggle_panel_autoclose();
                None
            }
            ExpandedHit::ToggleCapture => {
                self.run_action(PanelAction::ToggleCapture);
                None
            }
            ExpandedHit::CycleSource => {
                self.panel.cycle_source(&self.clips);
                self.panel.sel = 0;
                self.panel.scroll = 0;
                None
            }
            ExpandedHit::ClearUnpinned => {
                let action = if self.panel.clear_armed {
                    self.panel.clear_armed = false;
                    PanelAction::Clear
                } else {
                    self.panel.clear_armed = true;
                    PanelAction::ArmClear
                };
                self.run_action(action);
                None
            }
            ExpandedHit::Nav(n) => {
                self.panel.nav = n;
                self.panel.sel = 0;
                self.panel.scroll = 0;
                None
            }
            ExpandedHit::Row(i) => {
                self.panel.sel = i;
                None
            }
            ExpandedHit::Action(a) => {
                let id = self.panel.expanded_visible(&self.clips).get(self.panel.sel)?.id;
                match a {
                    ExpAction::Copy | ExpAction::QuickCopy => self.copy_text(id),
                    ExpAction::Pin => {
                        self.run_action(PanelAction::TogglePin(id));
                        None
                    }
                    ExpAction::Delete => {
                        self.run_action(PanelAction::Delete(id));
                        None
                    }
                    // Edit Note / Open Source: shown for parity with the design,
                    // no behavior yet (no per-clip notes, no source launching).
                    ExpAction::EditNote | ExpAction::OpenSource => None,
                }
            }
            ExpandedHit::None => None,
        }
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
        // In the expanded screen, Esc collapses back to the compact panel, and
        // Enter/Quick copy the selected clip without closing the screen.
        if self.panel.expanded {
            if key == NavKey::Esc {
                self.relayout(|p| p.panel.toggle_expanded());
                return None;
            }
            let action = self.panel.nav(key, &self.clips)?;
            if let PanelAction::Copy(id) = action {
                return self.copy_text(id);
            }
            return self.run_action(action);
        }
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
        // keys/sec over a ~1s window (decaying accumulator, time-constant 1s):
        // each tick adds the key count and decays, so it settles at the rate.
        self.kps = self.kps * (-dt).exp() + k as f32;

        let idle_secs = (now - self.last_event).as_secs_f32();
        let sleep_target = if idle_secs > SLEEP_AFTER { 1.0 } else { 0.0 };
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

        // typing-extreme sparks flicking off the paws (motion spec sparks:true)
        if self.kps >= KPS_EXTREME && t > self.spark_next {
            self.spark_next = t + 0.07;
            let r = rand_f(&mut self.rng);
            let left = r < 0.5;
            self.particles.push(Particle {
                x: if left { 92.0 } else { 150.0 } + (r - 0.5) * 10.0,
                y: 188.0,
                vx: (r - 0.5) * 60.0,
                vy: -40.0 - r * 20.0,
                life: 1.0,
                kind: ParticleKind::Sparkle,
                size: 3.5 + r * 2.0,
                spin: 0.0,
            });
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

        // XP popups drift up and fade
        for xp in self.xp_pops.iter_mut() {
            xp.y -= 22.0 * dt;
            xp.life -= dt / 1.2;
        }
        self.xp_pops.retain(|xp| xp.life > 0.0);

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
        let t = self.now_t();
        self.nom_until = t + NOM_SECS;
        self.happy_until = t + HAPPY_SECS;
        self.spawn_xp_pop(XP_PER_COPY as u32, 120.0, 108.0);
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
        // mouth opens as the fish closes in, fully open by the catch fraction
        // (motion spec mouth_open_at = 0.72)
        let mouth_open = match &self.fish {
            Some((_, ft)) => ((ft - 0.58) / 0.16).clamp(0.0, 1.0),
            None => 0.0,
        };

        // curious idle gestures: a slow side glance with the occasional ear
        // flick (motion spec random_gesture window), zero in every other mood.
        let (ear_twitch, look) = if self.mood() == PetMood::Curious {
            let flick = if (t % 5.0) < 0.22 { (t * 46.0).sin() * 5.0 } else { 0.0 };
            (flick, (t * 0.5).sin() * 2.6)
        } else {
            (0.0, 0.0)
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
            ear_twitch,
            look,
            accessory: Accessory::from_id(self.st.accessory),
            particles: &self.particles,
            xp_pops: &self.xp_pops,
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
            let caret = (t * 1.6).fract() < 0.65;
            if self.panel.expanded {
                let (lv, into, need) = level_progress(self.st.total_xp);
                let view = render::ExpandedView {
                    panel: &self.panel,
                    store: &self.clips,
                    lang: self.lang(),
                    version: env!("CARGO_PKG_VERSION"),
                    capture: self.st.clip_capture,
                    caret,
                    level: lv,
                    xp_into: into,
                    xp_need: need,
                    keys: self.st.keys_today,
                    clicks: self.st.clicks_today,
                    copies: self.st.copies_today,
                    autoclose: self.st.panel_autoclose,
                };
                render::draw_expanded_panel(pm, &view, self.scale());
            } else {
                let view = render::PanelView {
                    panel: &self.panel,
                    store: &self.clips,
                    lang: self.lang(),
                    capture: self.st.clip_capture,
                    hint: &self.panel_hint,
                    caret,
                };
                render::draw_panel(pm, &view, self.scale());
            }
        }
    }

    // ---- interactions ------------------------------------------------------

    /// Double-click / explicit pet: a burst of hearts and bonus XP.
    pub fn pet(&mut self) {
        self.happy = 1.0;
        self.petting_until = self.now_t() + PETTING_SECS;
        // A double-click arrives as click(+1) then this pet(): refund that
        // bounce so the gesture nets exactly +10 XP, not +11/+12.
        if let Some((bt, amt)) = self.last_bounce.take() {
            if self.now_t() - bt < DBLCLICK_REFUND {
                self.st.total_xp = self.st.total_xp.saturating_sub(amt);
            }
        }
        self.st.total_xp += 10;
        self.spawn_xp_pop(10, 120.0, 100.0);
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
        self.last_bounce = Some((self.now_t(), 1));
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

    /// Builds the right-click / tray context menu for the current state.
    /// `hotkey` is the live panel-hotkey label; `autostart` is the
    /// platform-queried "run at login/startup" state (the core does not track
    /// it). Mirrors the Windows tray menu order so the two stay at parity.
    pub fn build_menu(&self, hotkey: &str, autostart: bool) -> Vec<MenuEntry> {
        let lang = self.lang();
        let level = self.level();
        let mut m: Vec<MenuEntry> = Vec::new();

        // Update available (top, like the Windows tray).
        if let Some(ver) = self.update_available() {
            m.push(MenuItem::leaf(
                i18n::menu_update(lang, ver),
                MenuAction::InstallUpdate,
                false,
            ));
            m.push(MenuEntry::Separator);
        }

        m.push(MenuItem::leaf(
            i18n::menu_clipboard(lang, hotkey),
            MenuAction::TogglePanel,
            self.panel.open && !self.panel.expanded,
        ));
        m.push(MenuItem::leaf(
            t(lang, Msg::MenuExpanded),
            MenuAction::ToggleExpanded,
            self.panel.expanded,
        ));
        m.push(MenuItem::leaf(
            t(lang, Msg::MenuCapturePause),
            MenuAction::ToggleCapture,
            !self.st.clip_capture, // checked == capture is paused
        ));
        m.push(MenuItem::leaf(
            t(lang, Msg::MenuAutoClose),
            MenuAction::TogglePanelAutoClose,
            self.st.panel_autoclose,
        ));
        m.push(MenuEntry::Separator);

        m.push(MenuItem::leaf(
            t(lang, Msg::MenuShowStats),
            MenuAction::ToggleStats,
            self.st.bubble_pinned,
        ));

        // Size submenu.
        let sizes = [Msg::SizeSmall, Msg::SizeNormal, Msg::SizeLarge];
        let size_items = sizes
            .iter()
            .enumerate()
            .map(|(i, msg)| MenuItem::leaf(t(lang, *msg), MenuAction::SetSize(i), self.st.scale_idx == i))
            .collect();
        m.push(MenuItem::parent(t(lang, Msg::MenuSize), size_items));

        // Accessory submenu: None + each accessory, locked ones greyed.
        let mut acc_items = vec![MenuItem::leaf(
            t(lang, Msg::AccNone),
            MenuAction::SetAccessory(0),
            self.st.accessory == 0,
        )];
        for (i, acc) in ACCESSORIES.iter().enumerate() {
            let id = i + 1;
            let unlocked = level >= acc.level;
            let label = if unlocked {
                acc.name(lang).to_string()
            } else {
                i18n::accessory_locked(lang, acc.name(lang), acc.level)
            };
            acc_items.push(MenuEntry::Item(MenuItem {
                label,
                action: Some(MenuAction::SetAccessory(id)),
                checked: self.st.accessory == id,
                enabled: unlocked,
                submenu: Vec::new(),
            }));
        }
        m.push(MenuItem::parent(t(lang, Msg::MenuAccessory), acc_items));

        // Sound submenu.
        let sounds = [Msg::SoundOff, Msg::SoundEvents, Msg::SoundAll];
        let sound_items = sounds
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                MenuItem::leaf(t(lang, *msg), MenuAction::SetSound(i as u8), self.st.sound_mode as usize == i)
            })
            .collect();
        m.push(MenuItem::parent(t(lang, Msg::MenuSound), sound_items));

        m.push(MenuItem::leaf(
            t(lang, Msg::MenuLock),
            MenuAction::ToggleLock,
            self.st.locked,
        ));

        // Language submenu (labels are language-agnostic, like the Windows menu).
        let lang_items = vec![
            MenuItem::leaf("English", MenuAction::SetLang(Lang::En), lang == Lang::En),
            MenuItem::leaf("한국어", MenuAction::SetLang(Lang::Ko), lang == Lang::Ko),
        ];
        m.push(MenuItem::parent(t(lang, Msg::MenuLanguage), lang_items));

        m.push(MenuEntry::Separator);
        m.push(MenuItem::leaf(
            t(lang, Msg::MenuLoginStart),
            MenuAction::ToggleAutostart,
            autostart,
        ));
        m.push(MenuItem::leaf(
            t(lang, Msg::MenuAutoUpdate),
            MenuAction::ToggleAutoUpdate,
            self.st.auto_update,
        ));
        m.push(MenuItem::leaf(t(lang, Msg::MenuReset), MenuAction::ResetStats, false));
        m.push(MenuItem::leaf(t(lang, Msg::MenuAbout), MenuAction::About, false));
        m.push(MenuEntry::Separator);
        m.push(MenuItem::leaf(t(lang, Msg::MenuExit), MenuAction::Quit, false));
        m
    }

    /// Applies a chosen menu action. Pure state changes happen here and return
    /// [`MenuOutcome::Handled`]; the rest hand a [`MenuOutcome`] back to the
    /// backend (confirm/reset, about dialog, autostart, update page, quit).
    pub fn apply_menu_action(&mut self, action: MenuAction) -> MenuOutcome {
        match action {
            MenuAction::TogglePanel => self.toggle_panel(),
            MenuAction::ToggleExpanded => self.toggle_expanded(),
            MenuAction::ToggleCapture => {
                self.run_action(PanelAction::ToggleCapture);
            }
            MenuAction::TogglePanelAutoClose => self.toggle_panel_autoclose(),
            MenuAction::ToggleStats => {
                self.st.bubble_pinned = !self.st.bubble_pinned;
                self.dirty = true;
            }
            MenuAction::SetSize(i) => self.set_scale_idx(i),
            MenuAction::SetAccessory(id) => {
                // ignore a still-locked accessory (the menu greys it, but guard anyway)
                if id == 0 || self.level() >= ACCESSORIES[id - 1].level {
                    self.st.accessory = id;
                    self.dirty = true;
                }
            }
            MenuAction::SetSound(mode) => {
                self.st.sound_mode = mode.min(2);
                self.dirty = true;
            }
            MenuAction::ToggleLock => {
                self.st.locked = !self.st.locked;
                self.dirty = true;
            }
            MenuAction::SetLang(lang) => {
                self.st.set_lang(lang);
                self.dirty = true;
            }
            MenuAction::ToggleAutoUpdate => {
                self.st.auto_update = !self.st.auto_update;
                crate::update::set_enabled(self.st.auto_update);
                self.dirty = true;
            }
            MenuAction::ToggleAutostart => return MenuOutcome::ToggleAutostart,
            MenuAction::InstallUpdate => return MenuOutcome::InstallUpdate,
            MenuAction::ResetStats => return MenuOutcome::ConfirmReset,
            MenuAction::About => return MenuOutcome::ShowAbout,
            MenuAction::Quit => return MenuOutcome::Quit,
        }
        MenuOutcome::Handled
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
        self.levelup_until = t + LEVELUP_SECS;
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

    /// Float a "+N XP" popup up from (x, y).
    fn spawn_xp_pop(&mut self, amount: u32, x: f32, y: f32) {
        self.xp_pops.push(XpPop { x, y, life: 1.0, amount });
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
    fn fish_queue_overflow_merges_with_count() {
        let mut p = pet();
        for i in 0..(FISH_QUEUE_MAX + 2) {
            p.on_copy(format!("clip {i}"), Some("Code".into()), None);
        }
        // queue stays capped, but no copy is lost from the store...
        assert_eq!(p.fish_queue.len(), FISH_QUEUE_MAX);
        assert_eq!(p.clips.len(), FISH_QUEUE_MAX + 2);
        // ...the two overflow copies merge into the latest fish as a +N count.
        assert_eq!(p.fish_queue.back().unwrap().count, 3);
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

    /// Finds the first item (recursing into submenus) carrying `action`.
    fn find(entries: &[MenuEntry], action: MenuAction) -> Option<&MenuItem> {
        for e in entries {
            if let MenuEntry::Item(it) = e {
                if it.action == Some(action) {
                    return Some(it);
                }
                if let Some(found) = find(&it.submenu, action) {
                    return Some(found);
                }
            }
        }
        None
    }

    #[test]
    fn menu_radio_checks_follow_state() {
        let mut p = pet();
        let cur = p.st.scale_idx;
        assert!(find(&p.build_menu("HK", false), MenuAction::SetSize(cur)).unwrap().checked);

        let other = (cur + 1) % 3;
        assert_eq!(p.apply_menu_action(MenuAction::SetSize(other)), MenuOutcome::Handled);
        assert_eq!(p.st.scale_idx, other);
        let m = p.build_menu("HK", false);
        assert!(find(&m, MenuAction::SetSize(other)).unwrap().checked);
        assert!(!find(&m, MenuAction::SetSize(cur)).unwrap().checked);
    }

    #[test]
    fn menu_capture_toggle_marks_paused_and_toasts() {
        let mut p = pet();
        let on = p.st.clip_capture;
        // the item is checked when capture is *paused*
        assert_eq!(
            find(&p.build_menu("HK", false), MenuAction::ToggleCapture).unwrap().checked,
            !on
        );
        assert_eq!(p.apply_menu_action(MenuAction::ToggleCapture), MenuOutcome::Handled);
        assert_eq!(p.st.clip_capture, !on);
        assert!(p.toast.is_some(), "capture toggle shows a toast");
    }

    #[test]
    fn menu_autostart_check_reflects_param() {
        let p = pet();
        assert!(find(&p.build_menu("HK", true), MenuAction::ToggleAutostart).unwrap().checked);
        assert!(!find(&p.build_menu("HK", false), MenuAction::ToggleAutostart).unwrap().checked);
        // the toggle is a backend concern (write the LaunchAgent), so it defers
        let mut p = p;
        assert_eq!(p.apply_menu_action(MenuAction::ToggleAutostart), MenuOutcome::ToggleAutostart);
    }

    #[test]
    fn menu_locks_accessories_until_their_level() {
        let mut p = pet(); // level 1: every accessory locked
        let m = p.build_menu("HK", false);
        assert!(find(&m, MenuAction::SetAccessory(0)).unwrap().enabled, "None always enabled");
        assert!(!find(&m, MenuAction::SetAccessory(1)).unwrap().enabled, "locked accessory greyed");
        // applying a locked accessory is a guarded no-op
        p.apply_menu_action(MenuAction::SetAccessory(1));
        assert_eq!(p.st.accessory, 0);
    }

    #[test]
    fn menu_reset_waits_for_confirmation() {
        let mut p = pet();
        p.st.total_keys = 99;
        p.st.total_xp = 500;
        assert_eq!(p.apply_menu_action(MenuAction::ResetStats), MenuOutcome::ConfirmReset);
        assert_eq!(p.st.total_keys, 99, "ResetStats does not reset until the backend confirms");
        p.reset_stats();
        assert_eq!(p.st.total_keys, 0);
    }

    #[test]
    fn menu_update_item_appears_only_when_available() {
        let mut p = pet();
        assert!(find(&p.build_menu("HK", false), MenuAction::InstallUpdate).is_none());
        p.notify_update("9.9.9");
        assert!(find(&p.build_menu("HK", false), MenuAction::InstallUpdate).is_some());
        assert_eq!(p.apply_menu_action(MenuAction::InstallUpdate), MenuOutcome::InstallUpdate);
    }

    #[test]
    fn menu_backend_outcomes_do_not_mutate() {
        let mut p = pet();
        assert_eq!(p.apply_menu_action(MenuAction::About), MenuOutcome::ShowAbout);
        assert_eq!(p.apply_menu_action(MenuAction::Quit), MenuOutcome::Quit);
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
    fn panel_copy_returns_text_and_closes_panel() {
        let mut p = pet();
        p.on_copy("copy me back".into(), None, None);
        p.toggle_panel();
        assert!(p.panel_open());
        let got = p.panel_nav(NavKey::Enter);
        assert_eq!(got.as_deref(), Some("copy me back"));
        assert!(!p.panel_open(), "picking a clip closes the panel for pasting");
    }

    #[test]
    fn panel_delete_is_undoable_and_keeps_selection_sane() {
        let mut p = pet();
        p.on_copy("one".into(), None, None);
        p.on_copy("two".into(), None, None);
        p.toggle_panel();
        assert_eq!(p.panel_nav(NavKey::Delete), None); // deletes "two"
        assert_eq!(p.clips.len(), 1);
        assert!(p.toast.is_some(), "delete shows the undo hint");
        assert_eq!(p.panel_nav(NavKey::Undo), None);
        assert_eq!(p.clips.len(), 2, "Ctrl+Z restores the clip");
        assert_eq!(p.panel.sel, 0, "selection follows the restored clip");
    }

    #[test]
    fn panel_pin_key_keeps_selection_on_the_clip() {
        let mut p = pet();
        p.on_copy("old".into(), None, None);
        p.on_copy("new".into(), None, None);
        p.toggle_panel();
        p.panel_nav(NavKey::Down); // select "old"
        assert_eq!(p.panel_nav(NavKey::Pin), None);
        let visible = p.panel.visible(&p.clips);
        assert!(visible[p.panel.sel].pinned);
        assert_eq!(visible[p.panel.sel].text, "old", "selection follows the pin");
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
        let cat = p.panel.layout().cat;
        let (cx, cy) = p.cat_point(cat.0, cat.1);
        assert_eq!((cx, cy), (0.0, 0.0));
        // the window shifts so the cat itself never moves on screen
        assert_eq!(p.take_window_shift(), (-(cat.0 as i32), -(cat.1 as i32)));
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
    fn update_notification_toasts_once_per_version() {
        let mut p = pet();
        assert!(p.update_available().is_none());
        p.notify_update("9.9.9");
        assert_eq!(p.update_available(), Some("9.9.9"));
        assert!(p.toast.is_some());
        p.toast = None;
        p.notify_update("9.9.9"); // same version re-found: stay quiet
        assert!(p.toast.is_none());
        p.notify_update("9.9.10");
        assert_eq!(p.update_available(), Some("9.9.10"));
        assert!(p.toast.is_some());
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
