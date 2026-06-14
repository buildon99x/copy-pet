//! The platform-agnostic pet simulation: animation state machine, particle
//! system, XP/level progression, the copy-event fish, the clipboard store +
//! panel, and scene construction. Knows nothing about windows, hooks or
//! trays — the platform backends drive it by feeding input counts and copy
//! events, calling [`Pet::advance`] each tick and presenting [`Pet::render`].

use crate::clipboard::ClipStore;
use crate::i18n::{self, t, Lang, Msg};
use crate::menu::{MenuAction, MenuEntry, MenuItem, MenuOutcome};
use crate::panel::{NavKey, Panel, PanelAction, PanelDrag};
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

/// A clip the user picked from the panel, handed to the backend to put on the
/// OS clipboard. `paste` additionally asks the backend to paste it into the
/// previously focused app (synthesized Ctrl/Cmd+V; see `paste_on_select`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClipPick {
    pub text: String,
    pub paste: bool,
}

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
    update_available: Option<String>, // newer release found (crate::update)
    accessibility_hinted: bool,       // macOS Accessibility hint shown once
    // bookkeeping
    frame: u64,
    rng: u32,
    level: u32,
    last_min_bucket: u64,
    level_changed: bool,
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
        let (level, _, _) = level_progress(st.total_xp);
        let mut panel = Panel::with_geometry(st.panel_w, st.panel_h, (st.panel_off_x, st.panel_off_y));
        panel.cat_scale = SCALES[st.scale_idx.min(2)];
        panel.view = st.panel_view.min(1);
        Pet {
            st,
            clips: ClipStore::load(),
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
            update_available: None,
            accessibility_hinted: false,
            frame: 0,
            rng: seed(),
            level,
            last_min_bucket: 0,
            level_changed: false,
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

    /// The OS rejected the configured panel hotkey (e.g. Windows reserves
    /// Win+Shift+V for clipboard history) and the fallback chord took its
    /// place. Explain the swap via toast so the label shown in the menu/hint
    /// stops looking like a silent mismatch with the saved setting. The
    /// configured chord is kept in `state.json`, so it registers normally once
    /// whatever was holding it is freed.
    pub fn notify_hotkey_fallback(&mut self, wanted: &str, used: &str) {
        self.set_toast(i18n::hotkey_fallback(self.lang(), wanted, used), 5.0);
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
    /// When the panel is open the size comes straight from the layout, which
    /// is already physical (the card is fixed-scale, the cat block scaled).
    /// A flyout-owned panel lives in its own window, so the cat window keeps
    /// its plain cat-only size (see [`Pet::flyout_size`]).
    pub fn canvas_size(&self) -> (i32, i32) {
        if self.panel.open && !self.flyout {
            let l = self.panel.layout();
            (l.canvas_w.round() as i32, l.canvas_h.round() as i32)
        } else {
            window_size(self.scale())
        }
    }

    /// Top-left of the cat block inside the window canvas, in physical pixels.
    fn origin(&self) -> (f32, f32) {
        if self.panel.open && !self.flyout {
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
        let before = self.panel_anchor();
        f(self);
        let after = self.panel_anchor();
        self.pending_shift.0 += before.0 - after.0;
        self.pending_shift.1 += before.1 - after.1;
        self.size_changed = true;
    }

    /// Drags the cat by a screen-pixel delta while the panel is open: the cat
    /// moves but the card stays pixel-fixed. We shift the card's offset by
    /// `-delta` (the cat slides toward/away from the card) under a
    /// panel-anchored relayout, so the window re-origins to hold the card
    /// still. Persists the new offset. Returns false (a no-op) when the panel
    /// is closed — the backend then moves the whole window as usual.
    pub fn drag_pet(&mut self, dx: f32, dy: f32) -> bool {
        if !self.panel.open {
            return false;
        }
        self.drag = None;
        self.relayout_panel_anchored(|p| {
            p.panel.off.0 = (p.panel.off.0 - dx).clamp(-crate::panel::MAX_OFF, crate::panel::MAX_OFF);
            p.panel.off.1 = (p.panel.off.1 - dy).clamp(-crate::panel::MAX_OFF, crate::panel::MAX_OFF);
        });
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
            self.prev_level = self.st.window_level; // a visible level (not 2)
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
        self.panel.standalone = false;
        self.flyout = false;
        self.flyout_resized = false;
        self.drag = None;
    }

    /// Whether the panel is currently showing as a caret-anchored flyout.
    pub fn flyout_open(&self) -> bool {
        self.flyout && self.panel.open
    }

    /// Physical-pixel size of the flyout window (card + margins). The backend
    /// uses it to size/resize the flyout window; valid regardless of state.
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
        pm.fill(tiny_skia::Color::TRANSPARENT);
        render::draw_panel(pm, &self.build_panel_view(self.now_t()));
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

    /// Executes a panel action. Returns text the backend must put on the
    /// OS clipboard, if any.
    fn run_action(&mut self, action: PanelAction) -> Option<ClipPick> {
        match action {
            PanelAction::Copy(id) => {
                let text = self.clips.get(id).map(|c| c.text.clone());
                if let Some(text) = text {
                    self.set_toast(t(self.lang(), Msg::ToastCopied).to_string(), 1.6);
                    self.happy = (self.happy + 0.4).min(1.0);
                    if self.st.sound_mode >= 1 {
                        sound::play_pop();
                    }
                    // picked a clip: close the panel so the user can paste
                    // (switchable off to grab several clips in a row)
                    if self.st.panel_autoclose {
                        self.dismiss_panel();
                    }
                    return Some(ClipPick {
                        text,
                        paste: self.st.paste_on_select,
                    });
                }
                return None;
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

    /// Flips "paste the clip into the previous app after picking it" and
    /// confirms via toast.
    pub fn toggle_paste_on_select(&mut self) {
        self.st.paste_on_select = !self.st.paste_on_select;
        self.dirty = true;
        let msg = if self.st.paste_on_select {
            Msg::ToastPasteOn
        } else {
            Msg::ToastPasteOff
        };
        self.set_toast(t(self.lang(), msg).to_string(), 2.2);
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
    pub fn panel_nav(&mut self, key: NavKey) -> Option<ClipPick> {
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

        // first-run hint banner: the live panel hotkey, until the panel opens once
        let hint_text = self.show_hint().then(|| {
            let chord = crate::hotkey::Hotkey::from_spec(&self.st.hotkey).display();
            i18n::first_run_hint(self.lang(), &chord)
        });

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
            hotkey_hint: hint_text.as_deref(),
            lang: self.lang(),
            origin: self.origin(),
        };
        if card {
            render::render_card(pm, &scene, self.scale());
        } else {
            render::render(pm, &scene, self.scale());
        }
        // A flyout-owned panel is drawn in its own window, not here.
        if self.panel.open && !self.flyout {
            render::draw_panel(pm, &self.build_panel_view(t));
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
            self.panel.open,
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
        m.push(MenuItem::leaf(
            t(lang, Msg::MenuPasteOnSelect),
            MenuAction::TogglePasteOnSelect,
            self.st.paste_on_select,
        ));
        // Panel hotkey: a one-click cycle through safe presets (no rebind UI);
        // the label shows the live chord and a click advances to the next.
        let chord = crate::hotkey::Hotkey::from_spec(&self.st.hotkey).display();
        m.push(MenuItem::leaf(
            i18n::menu_hotkey(lang, &chord),
            MenuAction::CycleHotkey,
            false,
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

        // Window stacking submenu (always on top / normal / hide).
        let levels = [Msg::WinLevelTop, Msg::WinLevelNormal, Msg::WinLevelHide];
        let level_items = levels
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                MenuItem::leaf(t(lang, *msg), MenuAction::SetWindowLevel(i as u8), self.st.window_level as usize == i)
            })
            .collect();
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
        m.push(MenuItem::leaf(t(lang, Msg::MenuReset), MenuAction::ResetStats, false));
        m.push(MenuItem::leaf(t(lang, Msg::MenuAbout), MenuAction::About, false));
        m.push(MenuItem::leaf(t(lang, Msg::MenuGithub), MenuAction::OpenGithub, false));
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
            MenuAction::ToggleCapture => {
                self.run_action(PanelAction::ToggleCapture);
            }
            MenuAction::TogglePanelAutoClose => self.toggle_panel_autoclose(),
            MenuAction::TogglePasteOnSelect => self.toggle_paste_on_select(),
            MenuAction::ToggleStats => {
                self.st.bubble_pinned = !self.st.bubble_pinned;
                self.dirty = true;
            }
            MenuAction::SetSize(i) => self.set_scale_idx(i),
            MenuAction::SetAccessory(id) => {
                // ignore a still-locked or unknown accessory (the menu greys it,
                // but guard anyway — `id` is an untrusted index here)
                if id == 0 || ACCESSORIES.get(id - 1).is_some_and(|a| self.level() >= a.level) {
                    self.st.accessory = id;
                    self.dirty = true;
                }
            }
            MenuAction::SetSound(mode) => {
                self.st.sound_mode = mode.min(2);
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
            MenuAction::ResetStats => return MenuOutcome::ConfirmReset,
            MenuAction::About => return MenuOutcome::ShowAbout,
            MenuAction::OpenGithub => return MenuOutcome::OpenGithub,
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
    fn cycle_hotkey_advances_persists_and_toasts() {
        let mut p = pet();
        let before = p.st.hotkey.clone();
        let returned = p.cycle_hotkey();
        assert_eq!(p.st.hotkey, returned, "returns the new spec it persisted");
        assert_ne!(p.st.hotkey, before, "the spec advanced to the next preset");
        assert_eq!(p.st.hotkey, crate::hotkey::PRESETS[1]);
        assert!(p.dirty, "the new spec is persisted");
        assert!(p.toast.is_some(), "the new chord is confirmed via toast");
    }

    #[test]
    fn menu_cycle_hotkey_reregisters_with_new_spec() {
        let mut p = pet();
        // the menu carries a CycleHotkey leaf labelled with the live chord
        let menu = p.build_menu("HK", false);
        let item = find(&menu, MenuAction::CycleHotkey).unwrap();
        assert!(item.label.contains(&crate::hotkey::Hotkey::from_spec(&p.st.hotkey).display()));
        // applying it advances the spec and hands the backend the new one to register
        match p.apply_menu_action(MenuAction::CycleHotkey) {
            MenuOutcome::ReregisterHotkey(spec) => assert_eq!(spec, p.st.hotkey),
            other => panic!("expected ReregisterHotkey, got {other:?}"),
        }
    }

    #[test]
    fn paste_on_select_flag_flows_into_pick() {
        let mut p = pet();
        p.st.panel_autoclose = false; // keep the panel state simple
        p.on_copy("hello".into(), None, None);
        let id = p.clips.visible("")[0].id;
        p.toggle_panel();
        // default: copy only, no auto-paste
        let pick = p.run_action(PanelAction::Copy(id)).expect("a pick");
        assert_eq!(pick.text, "hello");
        assert!(!pick.paste, "auto-paste is off by default");
        // turning it on makes the next pick request a paste
        p.toggle_paste_on_select();
        assert!(p.paste_on_select());
        let pick = p.run_action(PanelAction::Copy(id)).expect("a pick");
        assert!(pick.paste, "now the pick also asks the backend to paste");
    }

    #[test]
    fn menu_toggle_paste_on_select() {
        let mut p = pet();
        let menu = p.build_menu("HK", false);
        assert!(!find(&menu, MenuAction::TogglePasteOnSelect).unwrap().checked);
        assert_eq!(
            p.apply_menu_action(MenuAction::TogglePasteOnSelect),
            MenuOutcome::Handled
        );
        assert!(p.paste_on_select());
        assert!(p.toast.is_some(), "the toggle is confirmed via toast");
        let menu = p.build_menu("HK", false);
        assert!(find(&menu, MenuAction::TogglePasteOnSelect).unwrap().checked);
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
    fn menu_has_a_github_link_under_about() {
        let mut p = pet();
        let m = p.build_menu("HK", false);
        // the GitHub item is present and sits directly after About
        let about = m
            .iter()
            .position(|e| matches!(e, MenuEntry::Item(i) if i.action == Some(MenuAction::About)))
            .unwrap();
        assert!(
            matches!(&m[about + 1], MenuEntry::Item(i) if i.action == Some(MenuAction::OpenGithub)),
            "GitHub should be the item right below About"
        );
        // opening it is OS work handed back to the backend
        assert_eq!(p.apply_menu_action(MenuAction::OpenGithub), MenuOutcome::OpenGithub);
    }

    #[test]
    fn window_level_menu_sets_state_and_restores_on_show() {
        let mut p = pet();
        // default is always-on-top (0): that radio item is checked
        assert!(find(&p.build_menu("HK", false), MenuAction::SetWindowLevel(0)).unwrap().checked);
        // choosing Hide stores the level and asks the backend to apply it
        assert_eq!(
            p.apply_menu_action(MenuAction::SetWindowLevel(2)),
            MenuOutcome::ApplyWindowLevel
        );
        assert_eq!(p.window_level(), 2);
        let m = p.build_menu("HK", false);
        assert!(find(&m, MenuAction::SetWindowLevel(2)).unwrap().checked);
        assert!(!find(&m, MenuAction::SetWindowLevel(0)).unwrap().checked);
        // show_window un-hides, restoring the level in effect before Hide (0)
        assert!(p.show_window());
        assert_eq!(p.window_level(), 0);
        assert!(!p.show_window(), "no-op when already visible");
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
        assert_eq!(got.map(|c| c.text).as_deref(), Some("copy me back"));
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
    fn opening_the_panel_requests_a_fit_once() {
        let mut p = pet();
        assert!(!p.take_fit_panel(), "closed panel: nothing to fit");
        p.toggle_panel(); // open
        assert!(p.take_fit_panel(), "opening asks the backend to fit on screen");
        assert!(!p.take_fit_panel(), "drained: only fired once");
        p.toggle_panel(); // close
        assert!(!p.take_fit_panel(), "closing never requests a fit");
    }

    #[test]
    fn open_panel_shows_and_never_closes() {
        let mut p = pet();
        assert!(!p.panel_open());
        p.open_panel(); // closed -> open
        assert!(p.panel_open());
        // pressing the hotkey again keeps it open (the hotkey only ever shows)
        p.open_panel();
        assert!(p.panel_open());
        // an in-flight search survives a re-show (not reset like a fresh open)
        p.panel_char('x');
        assert_eq!(p.panel.query, "x");
        p.open_panel();
        assert_eq!(p.panel.query, "x");
        assert!(p.panel_open());
    }

    #[test]
    fn shift_panel_moves_the_card_without_moving_the_cat() {
        let mut p = pet();
        p.toggle_panel();
        let _ = p.take_size_changed();
        let _ = p.take_window_shift(); // drain the open transition
        let off0 = p.panel.off;
        let anchor0 = p.cat_anchor();

        p.shift_panel(40.0, -30.0);
        assert_eq!(p.panel.off, (off0.0 + 40.0, off0.1 - 30.0), "card slides by the delta");
        assert_eq!((p.st.panel_off_x, p.st.panel_off_y), p.panel.off, "offset is persisted");
        assert!(p.take_size_changed(), "the canvas re-origins around the cat");

        // the window shift exactly cancels the cat's canvas move, so the cat
        // stays put on screen (anchor + shift is invariant)
        let (dx, dy) = p.take_window_shift();
        let anchor1 = p.cat_anchor();
        assert_eq!((anchor1.0 + dx as f32, anchor1.1 + dy as f32), anchor0);

        // a closed panel ignores the shift
        p.toggle_panel();
        let off = p.panel.off;
        p.shift_panel(10.0, 10.0);
        assert_eq!(p.panel.off, off);
    }

    #[test]
    fn panel_keeps_its_size_when_the_cat_scales() {
        let mut p = pet();
        p.toggle_panel();
        assert_eq!(p.scale(), 1.0, "normal is the default size");
        let normal = p.panel.layout();
        // grow the cat to the large size
        p.set_scale_idx(2);
        assert!(p.scale() > 1.0);
        let large = p.panel.layout();
        // the card itself is byte-identical — the panel never scales
        assert_eq!(
            (large.card_x, large.card_y, large.card_w, large.card_h, large.row_w, large.rows),
            (normal.card_x, normal.card_y, normal.card_w, normal.card_h, normal.row_w, normal.rows),
        );
        // ...but the window canvas grows on the cat's side
        assert!(large.canvas_w >= normal.canvas_w && large.canvas_h >= normal.canvas_h);
        assert!(large.canvas_w > normal.canvas_w || large.canvas_h > normal.canvas_h);
    }

    #[test]
    fn drag_pet_moves_the_cat_keeping_the_panel_fixed() {
        let mut p = pet();
        assert!(!p.drag_pet(10.0, 10.0), "closed panel: drag_pet is a no-op");
        p.toggle_panel();
        let _ = p.take_size_changed();
        let _ = p.take_window_shift(); // drain the open transition
        let off0 = p.panel.off;
        let anchor0 = p.panel_anchor();

        // dragging the cat shifts the card offset the opposite way
        assert!(p.drag_pet(-100.0, -320.0));
        assert_eq!(p.panel.off, (off0.0 + 100.0, off0.1 + 320.0));
        assert_eq!((p.st.panel_off_x, p.st.panel_off_y), p.panel.off, "offset persisted");
        assert!(p.take_size_changed());

        // the window shift exactly cancels the card's canvas move, so the card
        // stays pixel-fixed on screen while the cat slides
        let (dx, dy) = p.take_window_shift();
        let anchor1 = p.panel_anchor();
        assert_eq!((anchor1.0 + dx as f32, anchor1.1 + dy as f32), anchor0);
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
    fn first_panel_open_marks_onboarded() {
        let mut p = pet();
        assert!(!p.st.onboarded, "starts un-onboarded (first-run hint shown)");
        assert!(p.show_hint());
        p.toggle_panel(); // first open
        assert!(p.st.onboarded, "opening the panel once retires the hint");
        assert!(p.dirty, "the onboarding flag is persisted");
        assert!(!p.show_hint(), "panel open => no under-pet hint");
        p.toggle_panel(); // close again
        assert!(!p.show_hint(), "still onboarded after closing");
    }

    #[test]
    fn render_first_run_hint_smoke() {
        let p = pet(); // default => not onboarded
        assert!(p.show_hint());
        let (w, h) = p.canvas_size();
        let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
        p.render(&mut pm); // draws the hint banner; must not panic
        p.render_card(&mut pm);
        assert!(pm.data().chunks_exact(4).any(|px| px[3] > 0));
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
