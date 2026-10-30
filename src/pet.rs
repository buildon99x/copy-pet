//! The platform-agnostic pet simulation: animation state machine, particle
//! system, the copy-event fish, the clipboard store +
//! panel, and scene construction. Knows nothing about windows, hooks or
//! trays — the platform backends drive it by feeding copy
//! events, calling [`Pet::advance`] each tick and presenting [`Pet::render`].

use crate::clip_image::ClipImage;
use crate::clipboard::{ClipStore, RichFormats};
use crate::i18n::{self, t, Lang, Msg};
use crate::menu::{MenuAction, MenuEntry, MenuItem, MenuOutcome};
use crate::panel::{NavKey, Panel, PanelAction, PanelDrag};
use crate::render::{self, Badge, FishView, Particle, ParticleKind, Scene};
use crate::sound;
use crate::state::Persist;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tiny_skia::Pixmap;

/// Selectable pet sizes (small / normal / large) as canvas multipliers.
pub const SCALES: [f32; 3] = [0.78, 1.0, 1.3];

/// A clip the user picked from the panel, handed to the backend to put on the
/// OS clipboard. `paste` additionally asks the backend to paste it into the
/// previously focused app (synthesized Ctrl/Cmd+V; see `paste_on_select`).
///
/// `formats` carries the original rich formats to re-emit (ADR-0014); when
/// `plain_only` is set the backend writes plain text only — the "paste as text"
/// per-row action, which strips formatting regardless of what was captured.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClipPick {
    pub text: String,
    pub paste: bool,
    pub formats: Option<RichFormats>,
    pub plain_only: bool,
    pub image: Option<ClipImage>,
}

/// At most this many fish queue up during copy bursts.
const FISH_QUEUE_MAX: usize = 1;

/// Logical window size for a given selectable cat scale (cat only).
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

/// Builds a submenu of mutually-exclusive `msgs[i]` radio items, checking the
/// one at `current` and mapping each index through `mk_action` (e.g. to wrap
/// it in a `u8`-valued `MenuAction` variant).
fn radio_items(lang: Lang, msgs: &[Msg], current: usize, mk_action: impl Fn(usize) -> MenuAction) -> Vec<MenuEntry> {
    msgs.iter()
        .enumerate()
        .map(|(i, msg)| MenuItem::leaf(t(lang, *msg), mk_action(i), current == i))
        .collect()
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
    cat_art: crate::hunter_art::CatArt,
    pub cat_picker: Option<crate::cat_picker::CatPicker>,
    pub clips: ClipStore,
    pub workbench: crate::ai_workbench::Workbench,
    pub panel: Panel,
    pub dirty: bool,
    last_save: Instant,
    // timing
    start: Instant,
    last_tick: Instant,
    // animation
    paw_l: f32,
    paw_r: f32,
    blink_start: f32,
    blink_next: f32,
    happy: f32,
    sleep: f32,
    squash: f32,
    rate: f32,
    tail_phase: f32,
    breath_phase: f32,
    last_event: Instant,
    particles: Vec<Particle>,
    toast: Option<(String, f32)>, // text, expires_at (seconds since start)
    // copy-event fish
    fish: Option<(Badge, f32)>, // badge, flight progress 0..1
    fish_queue: VecDeque<Badge>,
    affection: Option<f32>,
    affection_pending: bool,
    cat_press: Option<(f32, f32)>,
    // inputs from the platform
    panel_hint: String,
    update_available: Option<String>, // newer release found (crate::update)
    // bookkeeping
    last_visual: Option<u64>,
    rng: u32,
    size_changed: bool,
    /// Set when the panel just opened; the backend drains it via
    /// [`Pet::take_fit_panel`] and nudges the card on-screen if it opened
    /// off the monitor (the core can't see screen bounds).
    fit_panel: bool,
    /// The window level to restore to when un-hiding (the level in effect
    /// before "Hide" was chosen). See [`Pet::show_window`].
    prev_level: u8,
    /// Window-position delta (physical px) accumulated by layout changes so
    /// the cat stays put on screen; drained via [`Pet::take_window_shift`].
    pending_shift: (f32, f32),
    /// Active panel-card drag (header move / grip resize), if any.
    drag: Option<PanelDrag>,
    /// The panel is showing in its own caret-anchored flyout window (hotkey
    /// path) rather than embedded in the cat window. While true the cat
    /// window draws no panel and keeps its plain cat-only size/origin — the
    /// cat never moves for the flyout. See [`Pet::open_flyout`].
    flyout: bool,
    /// Set when a flyout grip-resize changed the card size; the flyout window
    /// drains it via [`Pet::take_flyout_resized`] to rebuild its surface.
    flyout_resized: bool,
}

impl Pet {
    pub fn new(st: Persist) -> Pet {
        let now = Instant::now();
        let mut panel = Panel::with_geometry(st.panel_w, st.panel_h, (st.panel_off_x, st.panel_off_y));
        panel.cat_scale = SCALES[st.scale_idx.min(2)];
        panel.view = st.panel_view.min(1);
        let kind = crate::cat::CatKind::from_id(&st.cat_kind);
        let cat_art = crate::hunter_art::CatArt::load(kind).expect("bundled cat atlas");
        Pet {
            cat_art,
            cat_picker: None,
            st,
            clips: ClipStore::load(),
            workbench: crate::ai_workbench::Workbench::load(),
            panel,
            dirty: false,
            last_save: now,
            start: now,
            last_tick: now,
            paw_l: 0.0,
            paw_r: 0.0,
            blink_start: -1.0,
            blink_next: 2.0,
            happy: 0.0,
            sleep: 0.0,
            squash: 0.0,
            rate: 0.0,
            tail_phase: 0.0,
            breath_phase: 0.0,
            last_event: now,
            particles: Vec::new(),
            toast: None,
            fish: None,
            fish_queue: VecDeque::new(),
            affection: None,
            affection_pending: false,
            cat_press: None,
            panel_hint: String::new(),
            update_available: None,
            last_visual: None,
            rng: seed(),
            size_changed: false,
            fit_panel: false,
            prev_level: 0,
            pending_shift: (0.0, 0.0),
            drag: None,
            flyout: false,
            flyout_resized: false,
        }
    }

    pub fn scale(&self) -> f32 {
        SCALES[self.st.scale_idx.min(2)]
    }

    pub fn lang(&self) -> Lang {
        self.st.lang()
    }

    /// Excitement level derived from the recent input rate, used by both the
    /// tail-wag animation (`advance`) and the rendered scene (`draw`).
    fn excite(&self) -> f32 {
        (self.rate / 7.0).clamp(0.0, 1.0)
    }

    pub fn tooltip(&self) -> String {
        "ClipCat".into()
    }

    pub fn set_hover(&mut self, hover: bool) {
        let _ = hover;
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
        self.toast_msg(Msg::ToastUpdateDownloading, 120.0);
    }

    /// The update download/install failed; the menu entry stays for a retry.
    pub fn notify_update_failed(&mut self) {
        self.toast_msg(Msg::ToastUpdateFailed, 3.0);
    }

    /// Global registration failed; local panel gestures remain available.
    pub fn notify_hotkey_failed(&mut self) {
        self.toast_msg(Msg::ToastHotkeyFailed, 8.0);
    }

    /// The OS rejected the configured panel hotkey (e.g. Windows reserves
    /// Win+Shift+V for clipboard history) and the fallback chord took its
    /// place. Explain the swap via toast so the label shown in the menu/hint
    /// stops looking like a silent mismatch with the saved setting. The
    /// configured chord is kept in `state.json`, so it registers normally once
    /// whatever was holding it is freed.
    pub fn notify_hotkey_fallback(&mut self, wanted: &str, used: &str) {
        self.set_toast(i18n::hotkey_fallback(self.lang(), wanted, used), 5.0);
    }

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
    /// When the panel is open the size comes straight from the layout, which
    /// is already physical (the card is fixed-scale, the cat block scaled).
    /// A flyout-owned panel lives in its own window, so the cat window keeps
    /// its plain cat-only size (see [`Pet::flyout_size`]).
    pub fn canvas_size(&self) -> (i32, i32) {
        if self.embedded_panel_open() {
            let l = self.panel.layout();
            (l.canvas_w.round() as i32, l.canvas_h.round() as i32)
        } else {
            window_size(self.scale())
        }
    }

    /// Top-left of the cat block inside the window canvas, in physical pixels.
    fn origin(&self) -> (f32, f32) {
        if self.embedded_panel_open() {
            self.panel.layout().cat
        } else {
            (0.0, 0.0)
        }
    }

    /// The cat's bottom-center in physical pixels — the screen point every
    /// layout change keeps fixed (see [`Pet::take_window_shift`]). The origin
    /// is already physical; only the cat block itself is scaled.
    fn cat_anchor(&self) -> (f32, f32) {
        let (ox, oy) = self.origin();
        let s = self.scale();
        (ox + render::CANVAS_W * s / 2.0, oy + render::CANVAS_H * s)
    }

    /// Runs a canvas-layout mutation while keeping the screen point returned by
    /// `anchor` fixed: it accumulates the window shift that holds that point
    /// still across the change and flags the size change for the backend. The
    /// cat-anchored and panel-anchored relayouts differ only in `anchor`.
    fn relayout_with(&mut self, anchor: fn(&Pet) -> (f32, f32), f: impl FnOnce(&mut Pet)) {
        let before = anchor(self);
        f(self);
        let after = anchor(self);
        self.pending_shift.0 += before.0 - after.0;
        self.pending_shift.1 += before.1 - after.1;
        self.size_changed = true;
    }

    /// Runs a canvas-layout mutation, accumulating the window shift that
    /// keeps the cat anchored and flagging the size change for the backend.
    fn relayout(&mut self, f: impl FnOnce(&mut Pet)) {
        self.relayout_with(Pet::cat_anchor, f);
    }

    /// The panel card's top-left in physical pixels — the screen point a
    /// cat-body drag keeps fixed while the panel is open (the mirror of
    /// [`Pet::cat_anchor`]).
    fn panel_anchor(&self) -> (f32, f32) {
        let l = self.panel.layout();
        (l.card_x, l.card_y)
    }

    /// Like [`relayout`], but keeps the **panel card** fixed on screen instead
    /// of the cat — used when dragging the cat body so the card stays put.
    ///
    /// [`relayout`]: Pet::relayout
    fn relayout_panel_anchored(&mut self, f: impl FnOnce(&mut Pet)) {
        self.relayout_with(Pet::panel_anchor, f);
    }

    /// Drags the cat by a screen-pixel delta while the panel is open: the cat
    /// moves but the card stays pixel-fixed. We shift the card's offset by
    /// `-delta` (the cat slides toward/away from the card) under a
    /// panel-anchored relayout, so the window re-origins to hold the card
    /// still. Persists the new offset. Returns false (a no-op) when the
    /// embedded panel isn't open — the backend then moves the whole window as
    /// usual. This is deliberately gated on `embedded_panel_open()`, not on the
    /// raw `panel.open`: in flyout mode the panel lives in its own
    /// caret-anchored window and the cat window is cat-only, so the anchored
    /// relayout / persisted card offset here don't apply (it would jump the cat
    /// window by a bogus shift and rewrite the saved card offset).
    pub fn drag_pet(&mut self, dx: f32, dy: f32) -> bool {
        if !self.embedded_panel_open() {
            return false;
        }
        self.drag = None;
        // moving the cat by (dx, dy) under a panel-anchored relayout is the
        // card sliding by (-dx, -dy); drag_by applies the same MAX_OFF clamp.
        self.relayout_panel_anchored(|p| p.panel.drag_by(PanelDrag::Move, -dx, -dy));
        self.panel.refresh(&self.clips);
        self.st.panel_off_x = self.panel.off.0;
        self.st.panel_off_y = self.panel.off.1;
        self.dirty = true;
        true
    }

    /// Maps window coords (physical pixels) to cat-local canvas coords (for
    /// click_bounce). The cat block is scaled, so divide by the cat scale.
    pub fn cat_point(&self, px: f32, py: f32) -> (f32, f32) {
        let (ox, oy) = self.origin();
        let s = self.scale();
        ((px - ox) / s, (py - oy) / s)
    }

    pub fn set_scale_idx(&mut self, idx: usize) {
        if idx != self.st.scale_idx && idx < SCALES.len() {
            self.relayout(|p| {
                p.st.scale_idx = idx;
                p.panel.cat_scale = SCALES[idx];
            });
            self.dirty = true;
        }
    }

    // ---- window stacking level (0 top / 1 normal / 2 hidden) ----------------

    /// The persisted window level the backend should enforce.
    pub fn window_level(&self) -> u8 {
        self.st.window_level
    }

    /// Sets the window level, remembering the previous visible level so a
    /// later [`Pet::show_window`] can restore it. The backend applies the
    /// effect (topmost / normal / hide) on a `MenuOutcome::ApplyWindowLevel`.
    pub fn set_window_level(&mut self, level: u8) {
        let level = level.min(2);
        if level == self.st.window_level {
            return;
        }
        if level == 2 {
            self.affection = None;
            self.affection_pending = false;
            self.cat_press = None;
            self.prev_level = self.st.window_level; // a visible level (not 2)
            self.fish = None;
            self.fish_queue.clear();
            self.happy = 0.0;
            self.particles.clear();
            self.squash = 0.0;
        }
        self.st.window_level = level;
        self.dirty = true;
    }

    /// Un-hides the window if it was hidden, restoring the level in effect
    /// before "Hide". Returns true when it changed (the backend then re-applies
    /// the level). Used by the tray and the global panel hotkey.
    pub fn show_window(&mut self) -> bool {
        if self.st.window_level == 2 {
            self.st.window_level = self.prev_level;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    // ---- clipboard ----------------------------------------------------------

    /// A copy was observed system-wide. Stores the clip, feeds the cat a
    /// fish. `badge` lets the backend attach a real app icon;
    /// when None one is derived from the source app name.
    pub fn on_copy(&mut self, text: String, source: Option<String>, badge: Option<Badge>) {
        self.on_copy_rich(text, source, badge, None);
    }

    /// Like [`Pet::on_copy`], additionally storing the original rich formats the
    /// backend captured (ADR-0014). The plain `on_copy` is the `formats: None`
    /// case (Linux, or a plain copy).
    pub fn on_copy_rich(
        &mut self,
        text: String,
        source: Option<String>,
        badge: Option<Badge>,
        formats: Option<RichFormats>,
    ) {
        if !self.st.clip_capture {
            return;
        }
        let badge = badge.unwrap_or_else(|| Badge::from_source(source.as_deref()));
        if !self.clips.add_copy_rich(text, source, formats) {
            return;
        }
        self.copy_feedback(badge);
    }

    pub fn on_copy_image(&mut self, image: ClipImage, source: Option<String>, badge: Option<Badge>) {
        if !self.st.clip_capture { return; }
        let badge = badge.unwrap_or_else(|| Badge::from_source(source.as_deref()));
        if self.clips.add_image(image, source) { self.copy_feedback(badge); }
    }

    fn copy_feedback(&mut self, badge: Badge) {
        self.sleep = 0.0;
        if self.st.window_level != 2 {
            if self.fish_queue.len() >= FISH_QUEUE_MAX {
                self.fish_queue.pop_front();
            }
            self.fish_queue.push_back(badge);
        }
        self.dirty = true;
        self.last_event = Instant::now();
    }

    pub fn panel_open(&self) -> bool {
        self.panel.open
    }

    pub fn toggle_panel(&mut self) {
        // This toggle belongs to the cat window (middle-click / local menu).
        if self.flyout { self.close_flyout(); }
        if self.cat_picker.is_some() { self.close_cat_picker(); return; }
        self.drag = None;
        self.relayout(|p| {
            // Opening the panel for the first time retires the first-run hint.
            if !p.panel.open && !p.st.onboarded {
                p.st.onboarded = true;
                p.dirty = true;
            }
            p.panel.toggle();
        });
        // ask the backend to fit the card on screen once it's positioned —
        // a pet near a screen edge would otherwise open the panel offscreen
        if self.panel.open {
            self.fit_panel = true;
        }
    }

    /// Whether the first-run hotkey hint banner is currently shown: only while
    /// the panel is closed and the user has not yet opened it once.
    fn show_hint(&self) -> bool {
        !self.panel.open && !self.st.onboarded
    }

    /// Opens the panel if it isn't already — the global hotkey's "always show"
    /// (it never closes). A no-op on panel state when already open (the in-flight
    /// search/filter is preserved); the backend still brings the window to the
    /// front so an obscured or hidden panel reappears.
    pub fn open_panel(&mut self) {
        if self.cat_picker.is_some() { self.close_cat_picker(); }
        if !self.panel.open {
            self.toggle_panel();
        }
    }

    /// Returns `true` once after the panel opened, so the backend pulls the
    /// card on-screen if needed (see [`Pet::shift_panel`]). Only an *open*
    /// transition sets it, never a drag, so the card can still be parked
    /// partly off-screen by hand.
    pub fn take_fit_panel(&mut self) -> bool {
        std::mem::take(&mut self.fit_panel)
    }

    // ---- caret-anchored flyout window (hotkey path) --------------------------

    /// Opens the panel as a caret-anchored flyout in its **own** window (the
    /// global hotkey path). Unlike [`open_panel`](Pet::open_panel) this never
    /// relayouts the cat canvas — the cat stays exactly where it is and draws
    /// no panel; the backend positions the flyout window at the focused app's
    /// text caret. Idempotent on panel contents when already a flyout.
    pub fn open_flyout(&mut self) {
        if self.cat_picker.is_some() { self.close_cat_picker(); }
        // An embedded (middle-click) panel must retract first so the cat
        // window returns to its plain size before the flyout takes over.
        if self.panel.open && !self.flyout {
            self.toggle_panel();
        }
        if !self.panel.open {
            // Opening the panel for the first time retires the first-run hint.
            if !self.st.onboarded {
                self.st.onboarded = true;
                self.dirty = true;
            }
            self.panel.toggle(); // opens + clears query/source/scroll/sel
        }
        self.panel.standalone = true;
        self.flyout = true;
        self.drag = None;
    }

    /// Closes the flyout. The cat window was never resized for it, so this
    /// needs no relayout.
    pub fn close_flyout(&mut self) {
        self.panel.open = false;
        self.panel.tag_edit = None;
        self.panel.standalone = false;
        self.flyout = false;
        self.flyout_resized = false;
        self.drag = None;
    }

    /// Whether the panel is currently showing as a caret-anchored flyout.
    pub fn flyout_open(&self) -> bool {
        self.flyout && self.panel.open
    }

    /// The panel is showing *embedded* in the cat window (middle-click), not as
    /// the separate flyout — the only case in which the cat window itself takes
    /// keyboard focus and grows its canvas. Backends key their cat-window
    /// focus/size/clamp logic off this so the flyout (its own window) never
    /// leaves the cat window focusable.
    pub fn embedded_panel_open(&self) -> bool {
        self.panel.open && !self.flyout
    }

    /// Logical size of the flyout window (card + margins). Backends apply
    /// their device density when allocating physical buffers and windows.
    pub fn flyout_size(&self) -> (i32, i32) {
        let l = self.panel.layout_standalone();
        (l.canvas_w.round() as i32, l.canvas_h.round() as i32)
    }

    /// Renders only the panel card (no cat) into `pm` on a transparent
    /// background, for the flyout window (Windows layered window / macOS
    /// CALayer present). `pm` must be sized to [`flyout_size`](Pet::flyout_size)
    /// and the panel must be in standalone mode (set by [`open_flyout`]).
    ///
    /// [`open_flyout`]: Pet::open_flyout
    pub fn render_flyout(&self, pm: &mut Pixmap) {
        self.render_flyout_at_density(pm, 1.0);
    }

    /// The panel keeps logical geometry while glyphs rasterize at device density.
    pub fn render_flyout_at_density(&self, pm: &mut Pixmap, density: f32) {
        pm.fill(tiny_skia::Color::TRANSPARENT);
        render::draw_panel_scaled(pm, &self.build_panel_view(self.now_t()), density);
    }

    /// Returns true once after a flyout grip-resize changed the card size, so
    /// the flyout window rebuilds its surface to the new [`flyout_size`].
    ///
    /// [`flyout_size`]: Pet::flyout_size
    pub fn take_flyout_resized(&mut self) -> bool {
        std::mem::take(&mut self.flyout_resized)
    }

    /// Closes the panel however it is currently shown: the flyout window (no
    /// cat relayout) or the embedded cat-window panel. Used by clip-pick
    /// auto-close and the Close action so both panel modes dismiss correctly.
    fn dismiss_panel(&mut self) {
        if self.flyout {
            self.close_flyout();
        } else {
            self.toggle_panel();
        }
    }

    /// Slides the open panel card by a delta in canvas units **without moving
    /// the cat** — it re-origins the canvas and flags the window shift exactly
    /// like a header drag, then persists the new offset. The backend uses it
    /// (after [`Pet::take_fit_panel`]) to bring a card that opened off the
    /// monitor back into view. No-op when the panel is closed or the delta is
    /// zero.
    pub fn shift_panel(&mut self, dx: f32, dy: f32) {
        if !self.panel.open || (dx == 0.0 && dy == 0.0) {
            return;
        }
        self.relayout(|p| p.panel.drag_by(PanelDrag::Move, dx, dy));
        self.panel.refresh(&self.clips);
        self.st.panel_off_x = self.panel.off.0;
        self.st.panel_off_y = self.panel.off.1;
        self.dirty = true;
    }

    // ---- panel card drag (move / resize) -------------------------------------

    /// Begins a card move/resize drag when (cx, cy) — window-canvas coords —
    /// hits the card's header strip or its resize grip. Returns true when a
    /// drag started (the backend then feeds deltas and must not treat the
    /// press as a click).
    pub fn panel_drag_start(&mut self, cx: f32, cy: f32) -> bool {
        if self.cat_picker.is_some() { return false; }
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
        if self.flyout {
            // The flyout owns its window: a Move is the backend's job (it
            // repositions the window), and the card offset is never tracked.
            // Only a Resize changes core card state (size is shared/persisted).
            if kind == PanelDrag::Resize {
                self.panel.drag_by(kind, dx, dy);
                self.panel.refresh(&self.clips);
                self.st.panel_w = self.panel.w;
                self.st.panel_h = self.panel.h;
                self.dirty = true;
                self.flyout_resized = true;
                self.sync_workbench();
            }
            return;
        }
        self.relayout(|p| p.panel.drag_by(kind, dx, dy));
        // a shorter card may need the scroll re-clamped
        self.panel.refresh(&self.clips);
        self.st.panel_w = self.panel.w;
        self.st.panel_h = self.panel.h;
        self.st.panel_off_x = self.panel.off.0;
        self.st.panel_off_y = self.panel.off.1;
        self.dirty = true;
        self.sync_workbench();
    }

    /// The kind of card drag in progress, if any. The flyout backends use it
    /// to tell a window-move drag (header) from a grip-resize drag.
    pub fn panel_drag_kind(&self) -> Option<PanelDrag> {
        self.drag
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

    /// Functional feedback after picking a clip, and optional panel dismissal.
    fn after_pick(&mut self, toast: Msg) {
        self.toast_msg(toast, 1.6);
        if self.st.sound_mode >= 1 {
            sound::play_pop();
        }
        // picked a clip: close the panel so the user can paste
        // (switchable off to grab several clips in a row)
        if self.st.panel_autoclose {
            self.dismiss_panel();
        }
    }

    /// Executes a panel action. Returns text the backend must put on the
    /// OS clipboard, if any.
    fn run_action(&mut self, action: PanelAction) -> Option<ClipPick> {
        match action {
            PanelAction::Copy(id) => {
                if self.workbench.adding {
                    if let Some(clip) = self.clips.get(id) {
                        self.workbench.add_clip(clip);
                    }
                    self.sync_workbench();
                    return None;
                }
                // default pick: preserve the original formatting (ADR-0014)
                if let Some(clip) = self.clips.get(id) {
                    // The caret-anchored flyout is the Win+V parity path: the
                    // hotkey opened it at the focused field's text caret and the
                    // backend captured that field as the paste target, so picking
                    // a clip there always pastes — independent of the embedded
                    // (middle-click) panel's opt-in `paste_on_select`.
                    let pick = ClipPick {
                        text: clip.text.clone(),
                        paste: self.st.paste_on_select || self.flyout,
                        formats: clip.formats.clone(),
                        plain_only: false,
                        image: clip.image.clone(),
                    };
                    self.after_pick(Msg::ToastCopied);
                    return Some(pick);
                }
                return None;
            }
            PanelAction::PasteText(id) => {
                if self.workbench.adding {
                    if let Some(clip) = self.clips.get(id) {
                        self.workbench.add_clip(clip);
                    }
                    self.sync_workbench();
                    return None;
                }
                // "paste as text": strip formatting and paste it explicitly
                if let Some(clip) = self.clips.get(id) {
                    if clip.image.is_some() { return None; }
                    let pick = ClipPick {
                        text: clip.text.clone(),
                        paste: true,
                        formats: None,
                        plain_only: true,
                        image: None,
                    };
                    self.panel.collapse_actions();
                    self.after_pick(Msg::ToastPastedPlain);
                    return Some(pick);
                }
                return None;
            }
            PanelAction::SetTags(id, input) => {
                if self.clips.set_tags(id, &input) {
                    self.panel.tag_edit = None;
                    self.panel.focus_id(&self.clips, id);
                    self.toast_msg(Msg::ToastTagsSaved, 1.8);
                } else {
                    self.toast_msg(Msg::ToastTagsInvalid, 3.0);
                }
            }
            PanelAction::TogglePin(id) => {
                self.clips.toggle_pin(id);
                // pinning re-orders the list; keep the selection on the clip
                self.panel.focus_id(&self.clips, id);
            }
            PanelAction::Delete(id) => {
                if self.clips.delete(id) {
                    self.panel.refresh(&self.clips);
                    self.toast_msg(Msg::ToastDeleted, 2.2);
                }
            }
            PanelAction::ArmClear => {
                self.toast_msg(Msg::ToastClearConfirm, 2.6);
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
                    self.toast_msg(Msg::ToastRestored, 1.8);
                }
            }
            PanelAction::ToggleCapture => {
                self.st.clip_capture = !self.st.clip_capture;
                if !self.st.clip_capture {
                    self.affection = None;
                    self.affection_pending = false;
                    self.cat_press = None;
                    self.fish = None;
                    self.fish_queue.clear();
                    self.happy = 0.0;
                    self.particles.clear();
                    self.squash = 0.0;
                }
                self.toggle_bool_toast(self.st.clip_capture, Msg::ToastCaptureOn, Msg::ToastCapturePaused);
            }
            PanelAction::ToggleLang => {
                let lang = self.lang().toggled();
                self.st.set_lang(lang);
                self.dirty = true;
            }
            PanelAction::ToggleView => self.toggle_panel_view(),
            PanelAction::Close => self.dismiss_panel(),
        }
        None
    }

    /// Advances the global panel hotkey to the next preset (see
    /// [`crate::hotkey::next_preset`]), persists it, and toasts the new chord.
    /// Returns the new spec so the backend can re-register the OS hotkey; the
    /// backend also sets the precise panel hint (Windows may fall back on a
    /// clash, so the displayed label is the backend's to confirm).
    pub fn cycle_hotkey(&mut self) -> String {
        let next = crate::hotkey::next_preset(&self.st.hotkey).to_string();
        self.st.hotkey = next.clone();
        self.dirty = true;
        let chord = crate::hotkey::Hotkey::from_spec(&next).display();
        self.set_toast(chord, 2.2);
        next
    }

    /// Whether picking a clip also pastes it into the previous app.
    pub fn paste_on_select(&self) -> bool {
        self.st.paste_on_select
    }

    fn toast_msg(&mut self, msg: Msg, secs: f32) {
        self.set_toast(t(self.lang(), msg).to_string(), secs);
    }

    fn toggle_bool_toast(&mut self, on: bool, on_msg: Msg, off_msg: Msg) {
        self.dirty = true;
        self.toast_msg(if on { on_msg } else { off_msg }, 2.2);
    }

    /// Flips "paste the clip into the previous app after picking it" and
    /// confirms via toast.
    pub fn toggle_paste_on_select(&mut self) {
        self.st.paste_on_select = !self.st.paste_on_select;
        let on = self.st.paste_on_select;
        self.toggle_bool_toast(on, Msg::ToastPasteOn, Msg::ToastPasteOff);
    }

    /// Flips "close the panel after copying a clip" and confirms via toast.
    pub fn toggle_panel_autoclose(&mut self) {
        self.st.panel_autoclose = !self.st.panel_autoclose;
        let on = self.st.panel_autoclose;
        self.toggle_bool_toast(on, Msg::ToastAutoCloseOn, Msg::ToastAutoCloseOff);
    }

    /// Switches the clipboard list between the compact list and the roomier
    /// rounded-box cards. The card size (and thus the window) is unchanged —
    /// only the per-row height — so the scroll is re-clamped, not relaid out.
    pub fn toggle_panel_view(&mut self) {
        self.st.panel_view = if self.st.panel_view == 0 { 1 } else { 0 };
        self.panel.view = self.st.panel_view;
        self.panel.refresh(&self.clips);
        self.dirty = true;
    }

    /// A click at window-canvas coords while the panel is open.
    pub fn panel_click(&mut self, cx: f32, cy: f32) -> Option<ClipPick> {
        if self.cat_picker.is_some() {
            let l = self.panel.layout();
            let action = self.cat_picker.as_mut().unwrap().click(cx-l.card_x, cy-l.card_y);
            match action {
                crate::cat_picker::Action::Apply(kind) => {
                    if self.select_cat(kind).is_ok() { self.close_cat_picker(); }
                    else { self.set_toast(i18n::picker(self.lang(), i18n::PickerText::Error).into(), 3.0); }
                },
                crate::cat_picker::Action::Close => self.close_cat_picker(),
                crate::cat_picker::Action::None => {},
            }
            self.last_visual = None;
            return None;
        }
        if !self.panel.hit(cx, cy) {
            return None;
        }
        let lt = self.panel.active_layout();
        if self.workbench.open {
            if self.panel.workbench_footer_hit(cx, cy, self.lang()) {
                self.workbench.nav(NavKey::Esc, self.lang(), lt.rows);
            } else if cx >= lt.row_x
                && cx <= lt.row_x + lt.row_w
                && cy >= lt.rows_y
                && cy < lt.rows_y + lt.row_h * lt.rows as f32
            {
                let row = self.workbench.scroll + ((cy - lt.rows_y) / lt.row_h) as usize;
                let text = self.workbench.click_row(row, self.lang(), lt.rows);
                self.sync_workbench();
                return text.map(|text| ClipPick {
                    text,
                    paste: false,
                    formats: None,
                    plain_only: true,
                    image: None,
                });
            } else if cy >= lt.btn_y && cy <= lt.btn_y + crate::panel::BTN {
                let on_button = |left: f32| cx >= left && cx <= left + crate::panel::BTN;
                if on_button(lt.btn_close_x) {
                    self.dismiss_panel();
                } else if on_button(lt.btn_lang_x) {
                    self.st.lang = self.lang().toggled().code().into();
                    self.dirty = true;
                } else if on_button(lt.btn_filter_x) {
                    self.workbench.choose_projects();
                }
            }
            self.sync_workbench();
            return None;
        }
        if self.panel.workbench_footer_hit(cx, cy, self.lang()) {
            self.open_workbench();
            return None;
        }
        let action = self.panel.click(cx, cy, &self.clips)?;
        self.run_action(action)
    }
    /// Mouse wheel over the panel (positive rows = scroll down).
    pub fn panel_wheel(&mut self, rows: i32) {
        if self.cat_picker.is_some() { return; }
        if self.workbench.open {
            self.workbench
                .wheel(rows, self.panel.active_layout().rows, self.lang());
            self.sync_workbench();
        } else {
            self.panel.wheel(rows, &self.clips);
        }
    }

    /// Printable character while the panel is open (search input).
    pub fn panel_char(&mut self, c: char) {
        if let Some(picker)=self.cat_picker.as_mut() {
            if ('1'..='3').contains(&c) { picker.preview_mode(c as u8-b'1'); self.last_visual=None; }
            return;
        }
        if self.workbench.open {
            self.workbench.input(c);
            self.sync_workbench();
        } else {
            self.panel.input_char(c);
        }
    }

    /// Navigation key while the panel is open.
    pub fn panel_nav(&mut self, key: NavKey) -> Option<ClipPick> {
        if let Some(picker)=self.cat_picker.as_mut() {
            match key {
                NavKey::Left => picker.navigate(-1), NavKey::Right => picker.navigate(1),
                NavKey::Up => picker.navigate(-4), NavKey::Down => picker.navigate(4),
                NavKey::Enter => { let kind=picker.selected; if self.select_cat(kind).is_ok() { self.close_cat_picker(); } },
                NavKey::Esc => self.close_cat_picker(), _=>{},
            }
            self.last_visual=None;
            return None;
        }
        if key == NavKey::Workbench {
            if self.workbench.open {
                self.workbench.toggle();
                self.sync_workbench();
            } else {
                self.open_workbench();
            }
            return None;
        }
        if self.workbench.open {
            let text = self
                .workbench
                .nav(key, self.lang(), self.panel.active_layout().rows);
            self.sync_workbench();
            return text.map(|text| ClipPick {
                text,
                paste: false,
                formats: None,
                plain_only: true,
                image: None,
            });
        }
        let action = self.panel.nav(key, &self.clips)?;
        self.run_action(action)
    }

    /// Opens the existing panel in explicit request-preparation mode.
    pub fn open_workbench(&mut self) {
        if self.cat_picker.is_some() { self.close_cat_picker(); }
        if !self.panel.open {
            self.toggle_panel();
        }
        self.workbench.open = true;
        self.workbench.adding = false;
        self.workbench.review = None;
        self.sync_workbench();
    }

    fn sync_workbench(&mut self) {
        self.workbench.text_width = self.panel.active_layout().row_w - 14.0;
        self.panel.ai_view = self
            .workbench
            .open
            .then(|| self.workbench.view(self.lang()));
        self.panel.ai_adding = self.workbench.adding;
    }

    pub fn workbench_editing(&self) -> bool {
        self.cat_picker.is_none() && self.panel.open && self.workbench.open && self.workbench.editing()
    }

    /// Receives an explicit paste while our editor has focus, never global input.
    pub fn workbench_paste(&mut self, text: &str) {
        if self.workbench_editing() {
            self.workbench.paste(text);
            self.sync_workbench();
        }
    }

    // ---- per-frame update --------------------------------------------------

    /// Advances the simulation one frame and returns a
    /// hint of whether the visuals changed enough to be worth presenting.
    /// Only local actions and copies affect the pet.
    pub fn advance(&mut self) -> bool {
        let now = Instant::now();
        let t = self.now_t();
        let dt = (now - self.last_tick).as_secs_f32().clamp(0.001, 0.1);
        self.last_tick = now;

        // ease animation params toward rest
        self.paw_l = (self.paw_l - dt * 10.0).max(0.0);
        self.paw_r = (self.paw_r - dt * 10.0).max(0.0);
        self.happy = (self.happy - dt / 1.8).max(0.0);
        self.squash = (self.squash - dt * 6.0).max(0.0);


        let idle_secs = (now - self.last_event).as_secs_f32();
        let sleep_target = if idle_secs > 75.0 { 1.0 } else { 0.0 };
        self.sleep += (sleep_target - self.sleep) * (dt * 1.5).min(1.0);
        self.sleep = self.sleep.clamp(0.0, 1.0);

        if t >= self.blink_next {
            self.blink_start = t;
            self.blink_next = t + 2.2 + rand_f(&mut self.rng) * 3.5;
        }

        let excite = self.excite();
        self.tail_phase += dt * (1.3 + excite * 5.0 + self.happy * 2.0 - self.sleep * 0.9);
        // Breathing is a phase accumulator (like tail_phase), not sin(t·ω): the
        // frequency drops as `sleep` ramps, and multiplying absolute elapsed
        // time by a changing ω rewrites the whole phase each frame — a violent
        // per-frame jump (∝ elapsed time) that made the cat shudder at the exact
        // moment it dozed off. Integrating ω·dt instead keeps the motion smooth
        // across the transition while still slowing the breath while asleep.
        self.breath_phase += dt * (2.2 - self.sleep * 1.2);

        // Affection starts at the end of an in-flight copy and precedes its queue.
        if let Some(elapsed) = &mut self.affection {
            *elapsed += dt;
            if *elapsed >= self.cat_kind().affection_duration() { self.affection = None; }
        }
        if self.fish.is_none() && self.affection.is_none() && self.affection_pending {
            self.affection_pending = false;
            self.affection = Some(0.);
        }

        // fish flight
        let mut fish_started = false;
        if self.fish.is_none() && self.affection.is_none() && !self.affection_pending {
            if let Some(badge) = self.fish_queue.pop_front() {
                self.fish = Some((badge, 0.0));
                fish_started = true;
            }
        }
        let duration = self.cat_kind().copy_duration();
        let catch = self.cat_kind().catch_progress();
        let mut nom_now = false;
        if let Some((_, ft)) = &mut self.fish {
            let before = *ft;
            if !fish_started { *ft += dt / duration; }
            // The seventh illustrated pose opens the mouth. Eat here, then
            // finish the remaining satisfaction/recovery frames in the same
            // short cycle, rather than replaying a slow recovery afterwards.
            nom_now = before < catch && *ft >= catch;
            if *ft >= 1.0 {
                self.fish = None;
            }
        }
        if nom_now { self.nom(); }

        // particle physics
        for p in self.particles.iter_mut() {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            match p.kind {
                ParticleKind::Heart => {
                    p.vy += (-26.0 - p.vy) * dt * 2.0;
                    p.life -= dt / 1.3;
                }
                ParticleKind::Sparkle => p.life -= dt / 0.7,
                ParticleKind::Zzz => p.life -= dt / 2.6,
            }
        }
        self.particles.retain(|p| p.life > 0.0);

        // expire toast
        if let Some((_, until)) = &self.toast {
            if *until <= t {
                self.toast = None;
            }
        }

        let visual = self.visual_stamp(t);
        let changed = self.last_visual.replace(visual) != Some(visual);
        changed && (self.st.window_level != 2 || self.flyout)
    }

    fn blink(&self, t: f32) -> f32 {
        let elapsed = t - self.blink_start;
        if (0.0..0.14).contains(&elapsed) {
            1.0 - ((elapsed / 0.07) - 1.0).abs()
        } else { 0.0 }
    }

    fn hunter_frame(&self, t: f32) -> usize {
        if let Some(elapsed) = self.affection { return self.cat_kind().affection_frame(elapsed); }
        self.cat_kind().frame(
            self.fish.as_ref().map(|(_, progress)| *progress),
            self.happy, self.sleep, self.blink(t),
        )
    }

    /// Hash only visible state. No rasterization, serialization or clipboard
    /// scanning is needed on stable ticks; a change invalidates both presenters.
    fn visual_stamp(&self, t: f32) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(picker) = &self.cat_picker { (picker.selected, picker.frame()).hash(&mut h); }
        (
            self.hunter_frame(t),
            self.st.clip_capture,
            &self.st.cat_kind,
            self.st.scale_idx,
            self.st.window_level,
            self.st.onboarded,
            &self.st.lang,
            &self.st.hotkey,
            self.panel.open,
            self.flyout,
        )
            .hash(&mut h);
        ((self.squash * 100.0).round() as u32).hash(&mut h);
        if let Some((text, until)) = &self.toast {
            text.hash(&mut h);
            (((until - t) / 0.4).clamp(0.0, 1.0) * 20.0).round().to_bits().hash(&mut h);
        }
        if let Some((badge, progress)) = &self.fish {
            badge.letter.hash(&mut h);
            badge.color.hash(&mut h);
            if *progress <= self.cat_kind().catch_progress() + 0.03 {
                ((progress * 30.0).round() as u32).hash(&mut h);
            }
        }
        for p in &self.particles {
            std::mem::discriminant(&p.kind).hash(&mut h);
            ((p.x * 2.0).round() as i32, (p.y * 2.0).round() as i32,
                (p.life * 20.0).round() as i32, (p.spin * 8.0).round() as i32).hash(&mut h);
        }
        if self.panel.open {
            let p = &self.panel;
            (&p.query, &p.source, &p.tag_edit, p.scroll, p.sel, p.clear_armed,
                p.expanded, p.view, p.ai_adding, self.clips.version()).hash(&mut h);
            p.tag_select_all.hash(&mut h);
            (p.w.to_bits(), p.h.to_bits(), p.off.0.to_bits(), p.off.1.to_bits()).hash(&mut h);
            p.cursor.map(|(x, y)| (x.to_bits(), y.to_bits())).hash(&mut h);
            self.panel_hint.hash(&mut h);
            ((t * 1.6).fract() < 0.65).hash(&mut h);
            if let Some(v) = &p.ai_view {
                (v.editing, v.follow_tail, &v.title, &v.input, &v.hint,
                    &v.rows, v.selected, v.scroll).hash(&mut h);
            }
        }
        h.finish()
    }

    /// The fish reached the mouth: crunch, sparkles, a happy bump.
    fn nom(&mut self) {
        self.happy = (self.happy + 0.5).min(1.0);
        self.squash = 0.6;
        let (mx, my) = self.cat_kind().mouth(self.hunter_frame(self.now_t()));
        self.spawn_sparkles(4, mx, my);
        let r = rand_f(&mut self.rng);
        self.particles.push(Particle {
            x: mx,
            y: my - 12.0,
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
    fn fish_view<'a>(&self, badge: &'a Badge, ft: f32) -> FishView<'a> {
        // A short arc from the front paws into the illustrated open mouth;
        // keeping it below the face avoids covering the blue eyes.
        let kind = self.cat_kind();
        let (sx, sy) = kind.fish_start();
        let (mx, my) = kind.mouth(kind.frame(Some(kind.catch_progress()), 0.0, 0.0, 0.0));
        let (cx, cy) = ((sx + mx) * 0.5 + 8.0, sy.max(my) + 4.0);
        let u = 1.0 - ft;
        let x = u * u * sx + 2.0 * u * ft * cx + ft * ft * mx;
        let (_, current_mouth_y) = kind.mouth(kind.frame(Some(ft * kind.catch_progress()), 0.0, 0.0, 0.0));
        // A lowered head can cross the future capture arc. Stay below the
        // currently displayed mouth while retaining the authored endpoint.
        let y = (u * u * sy + 2.0 * u * ft * cy + ft * ft * my).max(current_mouth_y);
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
        self.draw(pm, false, 1.0);
    }

    /// Draws onto an opaque card background (portable backend).
    pub fn render_card(&self, pm: &mut Pixmap) {
        self.draw(pm, true, 1.0);
    }

    /// Renders the complete logical scene directly into a native-density pixmap.
    pub fn render_at_density(&self, pm: &mut Pixmap, density: f32) {
        self.draw(pm, false, density);
    }

    /// Builds the scene and rasterizes it with the chosen background.
    fn draw(&self, pm: &mut Pixmap, card: bool, density: f32) {
        let t = self.now_t();
        let blink = self.blink(t);
        let excite = self.excite();
        let breath = self.breath_phase.sin();

        let toast_view = self
            .toast
            .as_ref()
            .map(|(s, until)| (s.as_str(), ((*until - t) / 0.4).min(1.0)));

        let catch = self.cat_kind().catch_progress();
        let fish = self
            .fish
            .as_ref()
            .filter(|(_, progress)| *progress <= catch + 0.03)
            .map(|(badge, progress)| self.fish_view(badge, (progress / catch).min(1.0)));
        // mouth opens as the fish closes in
        let mouth_open = match &self.fish {
            Some((_, ft)) => ((ft - 0.45) / 0.4).clamp(0.0, 1.0),
            None => 0.0,
        };

        // first-run hint banner: the live panel hotkey, until the panel opens once
        let hint_text = self.show_hint().then(|| {
            i18n::first_run_hint(self.lang(), &self.panel_hint)
        });

        let scene = Scene {
            cat_art: &self.cat_art,
            hunter_frame: self.hunter_frame(t),
            capture_paused: !self.st.clip_capture,
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
            particles: &self.particles,
            fish,
            toast: toast_view,
            hotkey_hint: hint_text.as_deref(),
            lang: self.lang(),
            origin: self.origin(),
        };
        if card {
            render::render_card(pm, &scene, self.scale());
        } else {
            render::render_at_density(pm, &scene, self.scale(), density);
        }
        // A flyout-owned panel is drawn in its own window, not here.
        if self.panel.open && !self.flyout {
            if let Some(picker) = &self.cat_picker {
                let l = self.panel.layout();
                picker.draw_scaled(pm, (l.card_x, l.card_y), self.lang(), density);
            } else { render::draw_panel_scaled(pm, &self.build_panel_view(t), density); }
        }
    }

    /// The panel render view (caret blink derived from `t`), shared by the
    /// embedded cat-window [`draw`](Pet::draw) and the flyout window's
    /// [`render_flyout`](Pet::render_flyout).
    fn build_panel_view(&self, t: f32) -> render::PanelView<'_> {
        render::PanelView {
            panel: &self.panel,
            store: &self.clips,
            lang: self.lang(),
            capture: self.st.clip_capture,
            hint: &self.panel_hint,
            caret: (t * 1.6).fract() < 0.65,
        }
    }

    // ---- interactions ------------------------------------------------------

    /// Explicit local silhouette click, never called by global input observation.
    pub fn pet(&mut self) {
        if self.st.window_level == 2 { return; }
        self.sleep = 0.;
        self.last_event = Instant::now();
        if self.affection.is_none() && self.fish.is_none() {
            self.affection = Some(0.);
        } else {
            self.affection_pending = true;
        }
    }

    /// Tests the rendered character alpha, excluding panel and transparent space.
    pub fn cat_hit(&self, cx: f32, cy: f32) -> bool {
        if self.st.window_level == 2
            || (self.embedded_panel_open() && self.panel_hit(cx, cy)) { return false; }
        let (x, mut y) = self.cat_point(cx, cy);
        let squash = 1. - 0.035 * self.squash;
        y = 232. + (y - 232.) / squash;
        self.cat_art.hit(self.hunter_frame(self.now_t()), x, y)
    }
    pub fn cat_press(&mut self, x: f32, y: f32) -> bool {
        self.cat_press = self.cat_hit(x,y).then_some((x,y));
        self.cat_press.is_some()
    }
    /// Input is logical window points, independent of cat scale and Retina density.
    pub fn cat_motion(&mut self, x: f32, y: f32) {
        if self.cat_press.is_some_and(|(px,py)| (x-px).powi(2)+(y-py).powi(2)>9.) {
            self.cancel_cat_press();
        }
    }
    pub fn cancel_cat_press(&mut self) { self.cat_press = None; }
    pub fn cat_release(&mut self, x: f32, y: f32) -> bool {
        self.cat_motion(x,y);
        let clicked = self.cat_press.take().is_some() && self.cat_hit(x,y);
        if clicked { self.pet(); }
        clicked
    }

    pub fn click_bounce(&mut self, cx: f32, cy: f32) {
        let (ox,oy) = self.origin();
        self.cat_release(ox + cx*self.scale(), oy + cy*self.scale());
    }

    pub fn cat_kind(&self) -> crate::cat::CatKind {
        crate::cat::CatKind::from_id(&self.st.cat_kind)
    }

    pub fn select_cat(&mut self, kind: crate::cat::CatKind) -> Result<(), String> {
        let art = crate::hunter_art::CatArt::load(kind)?;
        self.cat_art = art;
        self.affection = None;
        self.affection_pending = false;
        self.cat_press = None;
        self.st.cat_kind = kind.id().into();
        self.fish = None;
        self.fish_queue.clear();
        self.particles.clear();
        self.toast = None;
        self.happy = 0.0;
        self.sleep = 0.0;
        self.squash = 0.0;
        self.last_event = Instant::now();
        self.blink_start = -1.0;
        self.blink_next = self.now_t() + 3.0;
        self.last_visual = None;
        self.dirty = true;
        Ok(())
    }

    pub fn open_cat_picker(&mut self) {
        if self.cat_picker.is_some() { return; }
        if self.flyout { self.close_flyout(); }
        self.open_panel();
        let mut picker = crate::cat_picker::CatPicker::new(self.cat_kind());
        picker.previous_size = (self.panel.w, self.panel.h);
        self.relayout(|p| { p.panel.w = crate::cat_picker::WIDTH; p.panel.h = crate::cat_picker::HEIGHT; });
        self.cat_picker = Some(picker);
        self.last_visual = None;
        self.fit_panel = true;
    }

    fn close_cat_picker(&mut self) {
        if let Some(picker) = self.cat_picker.take() {
            self.relayout(|p| { p.panel.w = picker.previous_size.0; p.panel.h = picker.previous_size.1; });
            self.dismiss_panel();
        }
    }

    /// Builds the right-click / tray context menu for the current state.
    /// `hotkey` is the live panel-hotkey label; `autostart` is the
    /// platform-queried "run at login/startup" state (the core does not track
    /// it). Mirrors the Windows tray menu order so the two stay at parity.
    pub fn build_menu(&self, hotkey: &str, autostart: bool) -> Vec<MenuEntry> {
        let lang = self.lang();
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
            self.panel.open,
        ));
        m.push(MenuItem::leaf(crate::i18n::ai(lang, crate::i18n::AiText::Entry), MenuAction::OpenWorkbench, self.workbench.open));
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
        m.push(MenuItem::leaf(
            t(lang, Msg::MenuPasteOnSelect),
            MenuAction::TogglePasteOnSelect,
            self.st.paste_on_select,
        ));
        // Panel hotkey: a one-click cycle through safe presets (no rebind UI);
        // the label shows the live chord and a click advances to the next.
        let chord = hotkey.to_string();
        m.push(MenuItem::leaf(
            i18n::menu_hotkey(lang, &chord),
            MenuAction::CycleHotkey,
            false,
        ));
        m.push(MenuEntry::Separator);

        // Size submenu.
        let sizes = [Msg::SizeSmall, Msg::SizeNormal, Msg::SizeLarge];
        let size_items = radio_items(lang, &sizes, self.st.scale_idx, MenuAction::SetSize);
        m.push(MenuItem::parent(t(lang, Msg::MenuSize), size_items));

        m.push(MenuItem::leaf(i18n::picker(self.lang(), i18n::PickerText::Title), MenuAction::OpenCatPicker, self.cat_picker.is_some()));

        // Sound submenu.
        let sounds = [Msg::SoundOff, Msg::SoundEvents];
        let sound_items = radio_items(lang, &sounds, self.st.sound_mode as usize, |i| {
            MenuAction::SetSound(i as u8)
        });
        m.push(MenuItem::parent(t(lang, Msg::MenuSound), sound_items));

        // Window stacking submenu (always on top / normal / hide).
        let levels = [Msg::WinLevelTop, Msg::WinLevelNormal, Msg::WinLevelHide];
        let level_items = radio_items(lang, &levels, self.st.window_level as usize, |i| {
            MenuAction::SetWindowLevel(i as u8)
        });
        m.push(MenuItem::parent(t(lang, Msg::MenuWindowLevel), level_items));

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
        m.push(MenuItem::leaf(t(lang, Msg::MenuAbout), MenuAction::About, false));
        m.push(MenuItem::leaf(t(lang, Msg::MenuGithub), MenuAction::OpenGithub, false));
        m.push(MenuEntry::Separator);
        m.push(MenuItem::leaf(t(lang, Msg::MenuExit), MenuAction::Quit, false));
        m
    }

    /// Applies a chosen menu action. Pure state changes happen here and return
    /// [`MenuOutcome::Handled`]; the rest hand a [`MenuOutcome`] back to the
    /// backend (about dialog, autostart, update page, quit).
    pub fn apply_menu_action(&mut self, action: MenuAction) -> MenuOutcome {
        match action {
            MenuAction::TogglePanel => self.toggle_panel(),
            MenuAction::OpenWorkbench => self.open_workbench(),
            MenuAction::ToggleCapture => {
                self.run_action(PanelAction::ToggleCapture);
            }
            MenuAction::TogglePanelAutoClose => self.toggle_panel_autoclose(),
            MenuAction::TogglePasteOnSelect => self.toggle_paste_on_select(),
            MenuAction::SetSize(i) => self.set_scale_idx(i),
            MenuAction::OpenCatPicker => self.open_cat_picker(),
            MenuAction::SetSound(mode) => {
                self.st.sound_mode = mode.min(1);
                self.dirty = true;
            }
            MenuAction::SetWindowLevel(level) => {
                self.set_window_level(level);
                // the actual topmost/normal/hide is OS work — let the backend do it
                return MenuOutcome::ApplyWindowLevel;
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
            MenuAction::CycleHotkey => return MenuOutcome::ReregisterHotkey(self.cycle_hotkey()),
            MenuAction::ToggleAutostart => return MenuOutcome::ToggleAutostart,
            MenuAction::InstallUpdate => return MenuOutcome::InstallUpdate,
            MenuAction::About => return MenuOutcome::ShowAbout,
            MenuAction::OpenGithub => return MenuOutcome::OpenGithub,
            MenuAction::Quit => return MenuOutcome::Quit,
        }
        MenuOutcome::Handled
    }

    fn set_toast(&mut self, text: String, secs: f32) {
        let t = self.now_t();
        self.toast = Some((text, t + secs));
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
        (self.dirty || self.clips.dirty || self.workbench.dirty) && self.last_save.elapsed().as_secs() >= 30
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
        self.workbench.save();
        self.sync_workbench();
        self.dirty = false;
        self.last_save = Instant::now();
    }
}

#[cfg(test)]
#[path = "pet_tests.rs"]
mod tests;
