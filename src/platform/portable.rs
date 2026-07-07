//! Portable backend for macOS, Linux and (with `--features portable`) Windows.
//!
//! Windowing via `winit`, software presentation via `softbuffer`, global
//! keyboard/mouse activity via `rdev` on its own listener thread, and the
//! clipboard via `arboard` (a polling watcher thread; ADR-0005). The pet is
//! drawn on an opaque rounded "card" because `softbuffer` cannot carry
//! per-pixel alpha to the desktop compositor (the native Win32 backend keeps
//! the fully transparent, click-through look). See ADR-0001 / ADR-0003.
//!
//! Interactions: drag to move, double-click to pet, single-click to bounce,
//! middle-click to toggle the clipboard panel, hover to show today's stats,
//! and the global panel hotkey (Cmd+Shift+V on macOS, Super+Shift+V on
//! Linux by default — the configured `win` modifier maps to the OS super
//! key). The panel card itself is movable (drag its header strip) and
//! resizable (drag the bottom-right grip) independently of the cat.
//! Settings have no system tray here; when the window is focused
//! these keys apply: S size · A accessory · M sound · B stats bubble ·
//! L lock · G language · C clipboard panel · O auto-close after copy ·
//! V list/thumbnail view · U update download page · Q/Esc quit. While the panel is open the
//! keyboard drives it instead (type to search, arrows/Home/End + Enter,
//! Ctrl+0..9 quick-copies the badged top rows, Del deletes, Ctrl+Z undoes,
//! Ctrl+P pins — Cmd works too on macOS — Tab cycles the source-app
//! filter, Esc closes).
//!
//! Updates (ADR-0009): the shared daily release check (`crate::update`)
//! toasts when a newer version exists; `U` then opens the releases page in
//! the browser — no self-replacement on the portable backend. The check is
//! disabled via `auto_update` in `state.json`.
//!
//! Privacy note (ADR-0008): the global-input listener increments the activity
//! counters and additionally compares each key event against the one
//! configured hotkey chord, in memory, discarding it immediately — key
//! identities are never stored, logged, buffered or transmitted. The only
//! other key-shaped state is [`KeyGate`]'s held-keys set, kept solely to
//! ignore OS auto-repeat and dropped on release.
//!
//! macOS uses a bespoke CoreGraphics event tap ([`super::mac_input`]) rather
//! than `rdev::listen`: rdev translates every keypress to text via Text Input
//! Source APIs that hard-crash off the main thread on macOS 15 (LNR-0005).
//! Linux keeps `rdev::listen` (X11). If the macOS tap can't be created
//! (Accessibility permission missing) the app stays up and shows a hint.

use crate::hotkey::Hotkey;
use crate::input;
use crate::clipboard::RichFormats;
use crate::panel::{self, fit_delta, NavKey, Rect};
use crate::pet::{ClipPick, Pet};

/// A clipboard observation handed from the watcher thread to the main loop:
/// the plain text plus any rich formats the platform could read (macOS only;
/// Linux/arboard is text-only, so `formats` is always `None` there).
struct ClipCapture {
    text: String,
    formats: Option<RichFormats>,
}
use crate::state::{Persist, ACCESSORIES};
#[cfg(not(target_os = "macos"))]
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId, WindowLevel};

const TICK: Duration = Duration::from_millis(33);
const DBLCLICK: Duration = Duration::from_millis(350);
/// Clipboard poll cadence (arboard has no change notifications).
const CLIP_POLL: Duration = Duration::from_millis(400);

#[cfg(not(target_os = "macos"))]
type Surface = softbuffer::Surface<Rc<Window>, Rc<Window>>;
/// Text we just wrote to the clipboard ourselves; the watcher skips it once.
type Suppress = Arc<Mutex<Option<String>>>;

/// Cross-thread handles wired from the global-input listener into the app: the
/// hotkey-fired and permission-needed flags it raises, plus the channel the app
/// uses to push a newly cycled chord back to it.
struct InputSignals {
    panel_toggle: Arc<AtomicBool>,
    perm_needed: Arc<AtomicBool>,
    hk_tx: Sender<Hotkey>,
}

struct PortableApp {
    pet: Pet,
    window: Option<Rc<Window>>,
    // Presentation: softbuffer (opaque card) everywhere except macOS, which
    // uses a transparent CALayer presenter instead (ADR-0003 / LNR-0001).
    #[cfg(not(target_os = "macos"))]
    _context: Option<softbuffer::Context<Rc<Window>>>,
    #[cfg(not(target_os = "macos"))]
    surface: Option<Surface>,
    #[cfg(target_os = "macos")]
    presenter: Option<super::mac_present::Presenter>,
    pm: tiny_skia::Pixmap,
    w: u32,
    h: u32,
    last_frame: Instant,
    // clipboard
    clip_rx: Receiver<ClipCapture>,
    suppress: Suppress,
    /// Mirror of `st.clip_capture` for the watcher thread: while false the
    /// clipboard is not even read.
    capture_flag: Arc<AtomicBool>,
    /// Raised by the global-input thread when the panel hotkey chord fires.
    panel_toggle: Arc<AtomicBool>,
    /// Raised by the global-input thread when the (macOS) event tap could not
    /// be installed — Accessibility permission is not granted yet.
    perm_needed: Arc<AtomicBool>,
    /// Panel-hotkey display label (e.g. "CMD+SHIFT+V") for the context menu.
    #[cfg(target_os = "macos")]
    hotkey_label: String,
    /// Sends a new panel chord to the global-input thread when the user cycles
    /// the hotkey preset, so the listener matches the new combo immediately.
    hk_tx: Sender<Hotkey>,
    // interaction
    cursor: PhysicalPosition<f64>,
    mouse_down: bool,
    dragging: bool,
    /// A panel-card drag (header move / grip resize) is in progress; deltas
    /// are tracked in *screen* coordinates because the window itself moves
    /// and resizes under the pointer while the card is dragged.
    panel_dragging: bool,
    /// A cat-body drag while the panel is open: the cat slides and the card
    /// stays pixel-fixed. Tracked in screen coordinates (`drag_screen`), like
    /// the card drag, since the window moves under the pointer.
    cat_drag: bool,
    drag_screen: (f64, f64),
    press_pos: PhysicalPosition<f64>,
    last_click: Option<Instant>,
    focused: bool,
    /// Live keyboard modifiers (for Ctrl+P / Ctrl+Z / Ctrl+digits in the panel).
    mods: ModifiersState,
    /// The caret-anchored clipboard flyout's own window (macOS only — Win+V
    /// parity). The hotkey opens the panel here, at the text caret, leaving
    /// the cat window untouched. Created lazily on first use. Linux/Windows-
    /// portable keep the embedded (cat-window) panel for the hotkey.
    #[cfg(target_os = "macos")]
    flyout: Option<Flyout>,
}

/// The flyout panel's dedicated winit window + transparent presenter (macOS).
#[cfg(target_os = "macos")]
struct Flyout {
    window: Rc<Window>,
    presenter: Option<super::mac_present::Presenter>,
    pm: tiny_skia::Pixmap,
    w: u32,
    h: u32,
    cursor: PhysicalPosition<f64>,
    /// header-move drag (moves the window) / grip-resize drag (resizes the card)
    move_drag: bool,
    resize_drag: bool,
    drag_screen: (f64, f64),
}

/// Converts a `MouseScrollDelta` to a signed row count (positive = scroll down).
fn scroll_rows(delta: MouseScrollDelta) -> f32 {
    match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(p) => p.y as f32,
    }
}

/// Screen-space cursor: `window.outer_position() + local`.
fn window_screen_cursor(window: &Window, local: PhysicalPosition<f64>) -> (f64, f64) {
    let base = window
        .outer_position()
        .map(|p| (p.x as f64, p.y as f64))
        .unwrap_or((0.0, 0.0));
    (base.0 + local.x, base.1 + local.y)
}

/// The full rect (not the taskbar-excluded work area) of the monitor `window`
/// currently sits on, in screen pixels.
fn monitor_rect(window: &Window) -> Option<Rect> {
    let mon = window.current_monitor()?;
    let mp = mon.position();
    let ms = mon.size();
    Some(Rect {
        x: mp.x as f32,
        y: mp.y as f32,
        w: ms.width as f32,
        h: ms.height as f32,
    })
}

/// Screen rect of a panel card whose window sits at `(origin_x, origin_y)`.
fn card_rect(origin_x: f32, origin_y: f32, l: &panel::Layout) -> Rect {
    Rect {
        x: origin_x + l.card_x,
        y: origin_y + l.card_y,
        w: l.card_w,
        h: l.card_h,
    }
}

impl PortableApp {
    fn new(
        pet: Pet,
        clip_rx: Receiver<ClipCapture>,
        suppress: Suppress,
        capture_flag: Arc<AtomicBool>,
        signals: InputSignals,
        hotkey_label: String,
    ) -> Self {
        let (w, h) = pet.canvas_size();
        let _ = &hotkey_label; // used only by the macOS context menu
        let InputSignals {
            panel_toggle,
            perm_needed,
            hk_tx,
        } = signals;
        PortableApp {
            pet,
            window: None,
            #[cfg(not(target_os = "macos"))]
            _context: None,
            #[cfg(not(target_os = "macos"))]
            surface: None,
            #[cfg(target_os = "macos")]
            presenter: None,
            pm: tiny_skia::Pixmap::new(w as u32, h as u32).unwrap(),
            w: w as u32,
            h: h as u32,
            last_frame: Instant::now(),
            clip_rx,
            suppress,
            capture_flag,
            panel_toggle,
            perm_needed,
            #[cfg(target_os = "macos")]
            hotkey_label,
            hk_tx,
            cursor: PhysicalPosition::new(0.0, 0.0),
            mouse_down: false,
            dragging: false,
            panel_dragging: false,
            cat_drag: false,
            drag_screen: (0.0, 0.0),
            press_pos: PhysicalPosition::new(0.0, 0.0),
            last_click: None,
            focused: false,
            mods: ModifiersState::default(),
            #[cfg(target_os = "macos")]
            flyout: None,
        }
    }

    /// Resizes the window/buffers to the wanted size (scale or panel layout
    /// changed), shifted by `Pet::take_window_shift` so the cat stays put
    /// on screen, then repaints.
    fn apply_size(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let fit = self.pet.take_fit_panel();
        let (nw, nh) = self.pet.canvas_size();
        let (nw, nh) = (nw as u32, nh as u32);

        let (dx, dy) = self.pet.take_window_shift();
        if (dx, dy) != (0, 0) {
            if let Ok(pos) = window.outer_position() {
                window.set_outer_position(PhysicalPosition::new(pos.x + dx, pos.y + dy));
            }
        }
        // recreate buffers only when the size really changed — a pure card
        // move repaints in place
        if (nw, nh) != (self.w, self.h) {
            let _ = window.request_inner_size(PhysicalSize::new(nw, nh));
            self.w = nw;
            self.h = nh;
            self.pm = tiny_skia::Pixmap::new(nw, nh).unwrap();
            self.resize_surface();
        }
        self.paint();

        // The panel just opened: if the card landed off the monitor (the pet
        // sits near an edge, or a persisted offset put it offscreen), slide
        // the card — not the cat — back into view and re-apply once. `fit` is
        // already drained, so the recursive call can't loop.
        if fit && self.pet.panel_open() {
            if let Some((sdx, sdy)) = self.panel_fit_shift(&window) {
                self.pet.shift_panel(sdx, sdy);
                self.apply_size();
            }
        }
    }

    /// Canvas-unit shift that brings the open panel card fully onto the
    /// monitor it sits on, or None when it already fits. winit exposes the
    /// full monitor rect (not the taskbar-excluded work area), so the panel
    /// can still sit under a taskbar — acceptable; the win is keeping it on
    /// the screen at all.
    fn panel_fit_shift(&self, window: &Window) -> Option<(f32, f32)> {
        let win = window.outer_position().ok()?;
        // layout() is already in physical pixels (card at scale 1.0), so the
        // card's screen rect is just the window origin plus the card offset.
        let l = self.pet.panel.layout();
        let card = card_rect(win.x as f32, win.y as f32, &l);
        let vis = monitor_rect(window)?;
        let (dx, dy) = fit_delta(card, vis);
        (dx.abs() >= 0.5 || dy.abs() >= 0.5).then_some((dx, dy))
    }

    /// Cursor position in screen coordinates (window position + local
    /// cursor); window-local coordinates would feed back into themselves
    /// while a panel drag moves/resizes the window under the pointer.
    fn screen_cursor(&self) -> (f64, f64) {
        match self.window.as_ref() {
            Some(w) => window_screen_cursor(w, self.cursor),
            None => (0.0, 0.0),
        }
    }


    /// Resizes the presentation buffers to the current `w`×`h`. On macOS the
    /// view's backing layer follows the window automatically, so this is a
    /// no-op there (the next `paint` pushes a correctly sized image).
    #[cfg(not(target_os = "macos"))]
    fn resize_surface(&mut self) {
        if let (Some(surface), Some(w), Some(h)) =
            (self.surface.as_mut(), NonZeroU32::new(self.w), NonZeroU32::new(self.h))
        {
            let _ = surface.resize(w, h);
        }
    }

    #[cfg(target_os = "macos")]
    fn resize_surface(&mut self) {}

    /// Renders the pet into the pixmap and presents it. macOS draws onto a
    /// transparent canvas and pushes it to the window's CALayer (a free-
    /// floating, background-transparent pet); every other platform draws the
    /// opaque "card" and blits it with softbuffer (ADR-0003 / LNR-0001).
    #[cfg(target_os = "macos")]
    fn paint(&mut self) {
        self.pet.render(&mut self.pm);
        let scale = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0);
        if let Some(presenter) = self.presenter.as_mut() {
            presenter.present(&self.pm, scale);
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn paint(&mut self) {
        self.pet.render_card(&mut self.pm);
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let Ok(mut buf) = surface.buffer_mut() else {
            return;
        };
        // tiny-skia premultiplied RGBA over an opaque card => straight RGB.
        let src = self.pm.data();
        for (dst, px) in buf.iter_mut().zip(src.chunks_exact(4)) {
            *dst = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
        }
        let _ = buf.present();
    }

    /// Cursor in window-client physical pixels. The panel is hit-tested at
    /// scale 1.0 (its coords are physical) and `Pet::cat_point` divides by the
    /// cat scale itself, so neither needs pre-division here.
    fn client_xy(&self) -> (f32, f32) {
        (self.cursor.x as f32, self.cursor.y as f32)
    }

    fn save_position(&mut self) {
        if let Some(window) = &self.window {
            if let Ok(pos) = window.outer_position() {
                self.pet.save_pos(pos.x, pos.y);
                return;
            }
        }
        self.pet.save();
    }

    /// Applies the persisted window level: 0 = always on top, 1 = normal (can
    /// go behind other windows), 2 = hidden (restored from the tray on macOS
    /// or the global panel hotkey). Surfaced via the menu on macOS only.
    fn apply_window_level(&self) {
        let Some(window) = &self.window else {
            return;
        };
        match self.pet.window_level() {
            2 => window.set_visible(false),
            level => {
                window.set_visible(true);
                window.set_window_level(if level == 0 {
                    WindowLevel::AlwaysOnTop
                } else {
                    WindowLevel::Normal
                });
            }
        }
    }

    /// Brings the (possibly obscured or hidden) window to the front and gives
    /// it focus — the panel hotkey's "reveal" for the embedded panel. On macOS
    /// the hotkey opens the caret-anchored flyout instead (its own window), so
    /// this is only used on Linux/Windows-portable.
    #[cfg(not(target_os = "macos"))]
    fn reveal(&mut self) {
        if self.pet.show_window() {
            self.apply_window_level(); // was hidden -> show
        }
        if let Some(window) = &self.window {
            window.focus_window();
        }
    }

    /// Puts text on the OS clipboard (a clip picked from the panel).
    /// Puts a panel-picked clip on the OS clipboard (our own change is
    /// suppressed once by the watcher). When `pick.paste` (the `paste_on_select`
    /// setting), it also synthesizes the paste shortcut. Focus restoration is
    /// best-effort here: unlike the Windows-native backend, the portable stack
    /// can't reliably re-focus the previous app, so the paste lands in whatever
    /// is frontmost after the panel closes.
    fn set_clipboard(&self, pick: ClipPick) {
        // suppression is keyed on the text, which is always written, so it stays
        // valid whether or not we also write the rich formats (ADR-0014).
        if let Ok(mut guard) = self.suppress.lock() {
            *guard = Some(pick.text.clone());
        }
        #[cfg(target_os = "macos")]
        {
            // macOS: write text plus the original HTML/RTF, unless this is the
            // "paste as text" action (plain_only) which strips formatting.
            let formats = if pick.plain_only { None } else { pick.formats.as_ref() };
            super::mac_clipboard::write(&pick.text, formats);
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Linux: arboard is text-only — rich formats degrade to plain.
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(pick.text.clone());
            }
        }
        if pick.paste {
            paste_synthesize();
        }
    }

    /// Pushes a (already-persisted) hotkey spec to the global-input thread and
    /// refreshes the visible labels. The portable chord matcher registers
    /// exactly what it is given, so the displayed chord is always the new one.
    fn apply_new_hotkey(&mut self, spec: &str) {
        let hk = Hotkey::from_spec(spec);
        let _ = self.hk_tx.send(hk);
        self.pet.set_panel_hint(format!("{} / C", hk.display()));
        #[cfg(target_os = "macos")]
        {
            self.hotkey_label = hk.display();
        }
    }

    /// Cycles the panel hotkey to the next preset (the `K` shortcut / Windows
    /// tray parity): advances the persisted spec in the core, then re-registers.
    fn cycle_hotkey(&mut self) {
        let spec = self.pet.cycle_hotkey();
        self.apply_new_hotkey(&spec);
    }

    /// macOS right-click: build the platform-agnostic menu model, render it as
    /// a native NSMenu, and apply the chosen action. State changes happen in
    /// `Pet::apply_menu_action`; the outcomes that need OS work (confirm/reset,
    /// About dialog, autostart, update page, quit) are finished here.
    #[cfg(target_os = "macos")]
    fn show_context_menu(&mut self, event_loop: &ActiveEventLoop) {
        use crate::i18n::{about_text, t, Msg};
        use crate::menu::MenuOutcome;

        let Some(window) = self.window.clone() else {
            return;
        };
        let autostart = super::mac_autostart::is_enabled();
        let entries = self.pet.build_menu(&self.hotkey_label, autostart);
        let Some(action) = super::mac_menu::popup(&window, &entries) else {
            return;
        };
        let lang = self.pet.lang();
        match self.pet.apply_menu_action(action) {
            MenuOutcome::Handled => {}
            MenuOutcome::Quit => {
                self.save_position();
                event_loop.exit();
            }
            MenuOutcome::ConfirmReset => {
                if super::mac_dialogs::confirm(
                    t(lang, Msg::ResetTitle),
                    t(lang, Msg::ResetConfirm),
                    t(lang, Msg::ResetTitle),
                    t(lang, Msg::Cancel),
                ) {
                    self.pet.reset_stats();
                }
            }
            MenuOutcome::ShowAbout => {
                let body = about_text(
                    lang,
                    env!("CARGO_PKG_VERSION"),
                    &self.hotkey_label,
                    self.pet.level(),
                    self.pet.st.total_keys,
                    self.pet.clips.len(),
                );
                super::mac_dialogs::info(t(lang, Msg::MenuAbout), &body);
            }
            MenuOutcome::ToggleAutostart => {
                super::mac_autostart::set(!autostart);
            }
            MenuOutcome::ReregisterHotkey(spec) => self.apply_new_hotkey(&spec),
            MenuOutcome::ApplyWindowLevel => self.apply_window_level(),
            MenuOutcome::OpenGithub => crate::update::open_github(),
            MenuOutcome::InstallUpdate => crate::update::open_releases_page(),
        }
    }

    /// Applies a keyboard shortcut (panel closed).
    fn shortcut(&mut self, code: KeyCode, event_loop: &ActiveEventLoop) {
        match code {
            KeyCode::KeyS => {
                self.pet
                    .set_scale_idx((self.pet.st.scale_idx + 1) % 3);
            }
            KeyCode::KeyA => {
                // cycle through none + unlocked accessories only
                let level = self.pet.level();
                let mut next = self.pet.st.accessory;
                for _ in 0..=ACCESSORIES.len() {
                    next = (next + 1) % (ACCESSORIES.len() + 1);
                    if next == 0 || level >= ACCESSORIES[next - 1].level {
                        break;
                    }
                }
                self.pet.st.accessory = next;
                self.pet.dirty = true;
            }
            KeyCode::KeyM => {
                self.pet.st.sound_mode = (self.pet.st.sound_mode + 1) % 3;
                self.pet.dirty = true;
            }
            KeyCode::KeyB => {
                self.pet.st.bubble_pinned = !self.pet.st.bubble_pinned;
                self.pet.dirty = true;
            }
            KeyCode::KeyL => {
                self.pet.st.locked = !self.pet.st.locked;
                self.pet.dirty = true;
            }
            KeyCode::KeyG => {
                let lang = self.pet.lang().toggled();
                self.pet.st.set_lang(lang);
                self.pet.dirty = true;
            }
            KeyCode::KeyC => self.pet.toggle_panel(),
            KeyCode::KeyK => self.cycle_hotkey(),
            KeyCode::KeyO => self.pet.toggle_panel_autoclose(),
            KeyCode::KeyV => self.pet.toggle_panel_view(),
            KeyCode::KeyU => {
                // only meaningful once the update toast announced a version
                if self.pet.update_available().is_some() {
                    crate::update::open_releases_page();
                }
            }
            KeyCode::KeyQ | KeyCode::Escape => {
                self.save_position();
                event_loop.exit();
            }
            _ => {}
        }
    }

    /// Keyboard input while the panel is open: search + navigation.
    fn panel_key(&mut self, event: &winit::event::KeyEvent) {
        // Ctrl on Linux/Windows-portable, Cmd on macOS — accept either
        let ctrl = self.mods.control_key() || self.mods.super_key();
        if let Some(key) = nav_from_key(event, ctrl) {
            if let Some(pick) = self.pet.panel_nav(key) {
                self.set_clipboard(pick);
            }
            return;
        }
        self.forward_text_input(event, ctrl);
    }

    /// Forwards printable characters to the panel search box, skipping
    /// unhandled ctrl/cmd chords.
    fn forward_text_input(&mut self, event: &winit::event::KeyEvent, ctrl: bool) {
        if ctrl {
            return;
        }
        if let Some(txt) = &event.text {
            for c in txt.chars() {
                self.pet.panel_char(c);
            }
        }
    }
}

// ---- caret-anchored flyout (macOS only) ------------------------------------

#[cfg(target_os = "macos")]
impl PortableApp {
    fn flyout_id(&self) -> Option<WindowId> {
        self.flyout.as_ref().map(|f| f.window.id())
    }

    /// Opens the panel as a flyout at the text caret (else the mouse cursor),
    /// creating its window on first use, sizing it to the card and focusing it
    /// for the search box. The cat window is left untouched.
    fn open_flyout(&mut self, event_loop: &ActiveEventLoop) {
        self.pet.open_flyout();
        let (fw, fh) = self.pet.flyout_size();
        let (fw, fh) = (fw as u32, fh as u32);

        if self.flyout.is_none() {
            let attrs = Window::default_attributes()
                .with_title("ClipCat Clipboard")
                .with_inner_size(PhysicalSize::new(fw, fh))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_visible(false);
            let Ok(win) = event_loop.create_window(attrs) else {
                return;
            };
            let win = Rc::new(win);
            let presenter = super::mac_present::Presenter::new(&win);
            self.flyout = Some(Flyout {
                window: win,
                presenter,
                pm: tiny_skia::Pixmap::new(fw, fh).unwrap(),
                w: fw,
                h: fh,
                cursor: PhysicalPosition::new(0.0, 0.0),
                move_drag: false,
                resize_drag: false,
                drag_screen: (0.0, 0.0),
            });
        }
        self.resize_flyout_buffers();
        self.place_flyout(super::mac_caret::caret_screen_pos());
        if let Some(f) = self.flyout.as_ref() {
            f.window.set_visible(true);
            f.window.focus_window();
        }
        self.paint_flyout();
    }

    /// Rebuilds the flyout window/buffers to the current [`Pet::flyout_size`]
    /// (after a grip-resize or before opening). No-op when unchanged.
    fn resize_flyout_buffers(&mut self) {
        let (fw, fh) = self.pet.flyout_size();
        let (fw, fh) = (fw as u32, fh as u32);
        if let Some(f) = self.flyout.as_mut() {
            if (fw, fh) != (f.w, f.h) {
                let _ = f.window.request_inner_size(PhysicalSize::new(fw, fh));
                f.w = fw;
                f.h = fh;
                f.pm = tiny_skia::Pixmap::new(fw, fh).unwrap();
            }
        }
    }

    /// Positions the flyout window so its card lands at `anchor` (physical px,
    /// top-left), slid onto the monitor it falls on. Falls back near the
    /// top-left when no anchor is available.
    fn place_flyout(&mut self, anchor: Option<(f64, f64)>) {
        let l = self.pet.panel.layout_standalone();
        let Some(f) = self.flyout.as_ref() else {
            return;
        };
        let (ax, ay) = anchor.unwrap_or((40.0, 40.0));
        // window top-left so the card (at card_x/card_y inside it) hits anchor
        let mut wx = ax - l.card_x as f64;
        let mut wy = ay - l.card_y as f64;
        // park there first so current_monitor resolves the anchor's monitor
        f.window
            .set_outer_position(PhysicalPosition::new(wx as i32, wy as i32));
        if let Some(vis) = monitor_rect(&f.window) {
            let card = card_rect(wx as f32, wy as f32, &l);
            let (dx, dy) = fit_delta(card, vis);
            wx += dx as f64;
            wy += dy as f64;
        }
        f.window
            .set_outer_position(PhysicalPosition::new(wx as i32, wy as i32));
    }

    fn paint_flyout(&mut self) {
        let Some(f) = self.flyout.as_mut() else {
            return;
        };
        self.pet.render_flyout(&mut f.pm);
        let scale = f.window.scale_factor();
        if let Some(p) = f.presenter.as_mut() {
            p.present(&f.pm, scale);
        }
    }

    fn close_flyout(&mut self) {
        self.pet.close_flyout();
        if let Some(f) = self.flyout.as_ref() {
            f.window.set_visible(false);
        }
    }

    /// Screen cursor for a flyout drag (window origin + local cursor); the
    /// window moves/resizes under the pointer, so window-local deltas would
    /// feed back into themselves.
    fn flyout_screen_cursor(&self) -> (f64, f64) {
        let Some(f) = self.flyout.as_ref() else {
            return (0.0, 0.0);
        };
        window_screen_cursor(&f.window, f.cursor)
    }

    fn flyout_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => self.paint_flyout(),
            WindowEvent::Focused(false) | WindowEvent::CloseRequested => self.close_flyout(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(f) = self.flyout.as_mut() {
                    f.cursor = position;
                }
                self.pet.set_cursor(position.x as f32, position.y as f32);
                let (mv, rz) = self
                    .flyout
                    .as_ref()
                    .map(|f| (f.move_drag, f.resize_drag))
                    .unwrap_or((false, false));
                if mv || rz {
                    let sc = self.flyout_screen_cursor();
                    let ds = self.flyout.as_ref().map(|f| f.drag_screen).unwrap();
                    let (dx, dy) = (sc.0 - ds.0, sc.1 - ds.1);
                    if dx != 0.0 || dy != 0.0 {
                        if mv {
                            if let Some(f) = self.flyout.as_ref() {
                                if let Ok(p) = f.window.outer_position() {
                                    f.window.set_outer_position(PhysicalPosition::new(
                                        p.x + dx as i32,
                                        p.y + dy as i32,
                                    ));
                                }
                            }
                        } else {
                            self.pet.panel_drag_update(dx as f32, dy as f32);
                        }
                        if let Some(f) = self.flyout.as_mut() {
                            f.drag_screen = sc;
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => {
                    let (cx, cy) = self
                        .flyout
                        .as_ref()
                        .map(|f| (f.cursor.x as f32, f.cursor.y as f32))
                        .unwrap_or((0.0, 0.0));
                    if self.pet.panel_drag_start(cx, cy) {
                        let kind = self.pet.panel_drag_kind();
                        let sc = self.flyout_screen_cursor();
                        if let Some(f) = self.flyout.as_mut() {
                            f.drag_screen = sc;
                            match kind {
                                Some(crate::panel::PanelDrag::Move) => f.move_drag = true,
                                Some(crate::panel::PanelDrag::Resize) => f.resize_drag = true,
                                None => {}
                            }
                        }
                        return;
                    }
                    if self.pet.panel_hit(cx, cy) {
                        let pick = self.pet.panel_click(cx, cy);
                        self.after_flyout_action(pick);
                    }
                }
                ElementState::Released => {
                    let dragging = self
                        .flyout
                        .as_ref()
                        .map(|f| f.move_drag || f.resize_drag)
                        .unwrap_or(false);
                    if dragging {
                        if let Some(f) = self.flyout.as_mut() {
                            f.move_drag = false;
                            f.resize_drag = false;
                        }
                        self.pet.panel_drag_end();
                    }
                }
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = scroll_rows(delta);
                if dy != 0.0 {
                    self.pet.panel_wheel(if dy < 0.0 { 1 } else { -1 });
                }
            }
            WindowEvent::Ime(Ime::Commit(s)) => {
                for c in s.chars() {
                    self.pet.panel_char(c);
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.flyout_key(&event);
                }
            }
            _ => {}
        }
    }

    /// Like [`panel_key`](Self::panel_key), but reconciles the flyout window
    /// (hide on dismiss, paste on pick) after each navigation key.
    fn flyout_key(&mut self, event: &winit::event::KeyEvent) {
        let ctrl = self.mods.control_key() || self.mods.super_key();
        if let Some(key) = nav_from_key(event, ctrl) {
            let pick = self.pet.panel_nav(key);
            self.after_flyout_action(pick);
            return;
        }
        self.forward_text_input(event, ctrl);
    }

    /// After a flyout click/key: hide the window if the panel closed (a pick
    /// with auto-close, or Esc), then hand any picked clip to the clipboard.
    fn after_flyout_action(&mut self, pick: Option<ClipPick>) {
        if !self.pet.flyout_open() {
            if let Some(f) = self.flyout.as_ref() {
                f.window.set_visible(false);
            }
        }
        if let Some(pick) = pick {
            self.set_clipboard(pick);
        }
    }
}

impl ApplicationHandler for PortableApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (w, h) = (self.w as i32, self.h as i32);

        // initial position: saved, else bottom-right of the primary monitor
        let pos = if self.pet.st.has_pos {
            PhysicalPosition::new(self.pet.st.pos_x, self.pet.st.pos_y)
        } else {
            let (mw, mh) = event_loop
                .primary_monitor()
                .map(|m| (m.size().width as i32, m.size().height as i32))
                .unwrap_or((1920, 1080));
            PhysicalPosition::new(mw - w - 40, mh - h - 64)
        };

        // macOS presents through a non-opaque CALayer (transparent, floating
        // pet); softbuffer platforms keep the opaque card (ADR-0003).
        let transparent = cfg!(target_os = "macos");
        let attrs = Window::default_attributes()
            .with_title("ClipCat")
            .with_inner_size(PhysicalSize::new(w as u32, h as u32))
            .with_position(pos)
            .with_decorations(false)
            .with_transparent(transparent)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);

        let window = match event_loop.create_window(attrs) {
            Ok(win) => Rc::new(win),
            Err(_) => {
                event_loop.exit();
                return;
            }
        };
        // Korean search input needs the IME (composition arrives as Ime::Commit)
        window.set_ime_allowed(true);

        #[cfg(not(target_os = "macos"))]
        {
            let context = softbuffer::Context::new(window.clone()).ok();
            let surface = context
                .as_ref()
                .and_then(|ctx| softbuffer::Surface::new(ctx, window.clone()).ok());
            self._context = context;
            self.surface = surface;
        }
        #[cfg(target_os = "macos")]
        {
            self.presenter = super::mac_present::Presenter::new(&window);
        }

        self.window = Some(window);
        self.resize_surface();
        self.last_frame = Instant::now();
        self.paint();
        self.apply_window_level(); // enforce a persisted level/hide
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // the caret-anchored flyout (macOS) is a second window: route its
        // events to the flyout handler, leaving the cat window's path below.
        #[cfg(target_os = "macos")]
        if self.flyout_id() == Some(id) {
            self.flyout_window_event(event);
            return;
        }
        #[cfg(not(target_os = "macos"))]
        let _ = id;
        match event {
            WindowEvent::CloseRequested => {
                self.save_position();
                event_loop.exit();
            }
            WindowEvent::Focused(f) => self.focused = f,
            WindowEvent::RedrawRequested => self.paint(),
            WindowEvent::CursorEntered { .. } => self.pet.set_hover(true),
            WindowEvent::CursorLeft { .. } => {
                self.pet.set_hover(false);
                self.pet.clear_cursor();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                let (cx, cy) = self.client_xy();
                self.pet.set_cursor(cx, cy);
                if self.panel_dragging || self.cat_drag {
                    // card drag / cat-body drag (mirror images of each other):
                    // screen-pixel deltas are panel units (1.0); the tick
                    // applies the new layout (resize + window shift)
                    let sc = self.screen_cursor();
                    let (dx, dy) = (sc.0 - self.drag_screen.0, sc.1 - self.drag_screen.1);
                    if dx != 0.0 || dy != 0.0 {
                        if self.panel_dragging {
                            self.pet.panel_drag_update(dx as f32, dy as f32);
                        } else {
                            self.pet.drag_pet(dx as f32, dy as f32);
                        }
                        self.drag_screen = sc;
                    }
                } else if self.mouse_down && !self.dragging && !self.pet.st.locked {
                    let dx = position.x - self.press_pos.x;
                    let dy = position.y - self.press_pos.y;
                    if dx * dx + dy * dy > 9.0 {
                        self.dragging = true;
                        self.mouse_down = false;
                        if self.pet.panel_open() {
                            // keep the card fixed; move the cat incrementally
                            // (drag_window() is a one-shot whole-window move,
                            // which would drag the card along — not wanted here)
                            self.cat_drag = true;
                            self.drag_screen = self.screen_cursor();
                        } else if let Some(window) = &self.window {
                            let _ = window.drag_window();
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Left => match state {
                    ElementState::Pressed => {
                        let (cx, cy) = self.client_xy();
                        // header strip / resize grip start a card drag (the
                        // grip pokes slightly past the card edge)
                        if self.pet.panel_drag_start(cx, cy) {
                            self.panel_dragging = true;
                            self.drag_screen = self.screen_cursor();
                            self.mouse_down = false;
                            self.last_click = None;
                            return;
                        }
                        if self.pet.panel_hit(cx, cy) {
                            // panel interactions act on press; no drag/petting
                            if let Some(pick) = self.pet.panel_click(cx, cy) {
                                self.set_clipboard(pick);
                            }
                            self.mouse_down = false;
                            self.last_click = None;
                            return;
                        }
                        let now = Instant::now();
                        let is_double = self
                            .last_click
                            .map(|t| now.duration_since(t) < DBLCLICK)
                            .unwrap_or(false);
                        if is_double {
                            self.pet.pet();
                            self.last_click = None;
                            self.mouse_down = false;
                        } else {
                            self.last_click = Some(now);
                            self.mouse_down = true;
                            self.dragging = false;
                            self.press_pos = self.cursor;
                        }
                    }
                    ElementState::Released => {
                        if self.panel_dragging {
                            self.panel_dragging = false;
                            self.pet.panel_drag_end();
                        } else if self.cat_drag {
                            self.cat_drag = false;
                        } else if self.mouse_down && !self.dragging {
                            let (cx, cy) = self.client_xy();
                            let (lx, ly) = self.pet.cat_point(cx, cy);
                            self.pet.click_bounce(lx, ly);
                        }
                        self.mouse_down = false;
                        self.dragging = false;
                    }
                },
                MouseButton::Middle => {
                    // toggle on release to match the Windows native backend
                    // (WM_MBUTTONUP), keeping the panel-toggle timing in parity.
                    if state == ElementState::Released {
                        self.pet.toggle_panel();
                    }
                }
                #[cfg(target_os = "macos")]
                MouseButton::Right => {
                    if state == ElementState::Pressed {
                        self.show_context_menu(event_loop);
                    }
                }
                _ => {}
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = scroll_rows(delta);
                let (cx, cy) = self.client_xy();
                // Only scroll the panel here. A wheel's "activity" is already
                // counted once by the global input listener (rdev / the macOS
                // event tap), so also counting it here double-counted scrolls
                // over the pet relative to scrolls over other apps.
                if self.pet.panel_hit(cx, cy) && dy != 0.0 {
                    self.pet.panel_wheel(if dy < 0.0 { 1 } else { -1 });
                }
            }
            WindowEvent::Ime(Ime::Commit(s)) => {
                if self.pet.panel_open() {
                    for c in s.chars() {
                        self.pet.panel_char(c);
                    }
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if self.pet.panel_open() {
                        self.panel_key(&event);
                    } else if let PhysicalKey::Code(code) = event.physical_key {
                        self.shortcut(code, event_loop);
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now.duration_since(self.last_frame) >= TICK {
            self.last_frame = now;
            // the global panel hotkey fired on the input thread
            if self.panel_toggle.swap(false, Ordering::Relaxed) {
                // macOS: open the panel as a caret-anchored flyout in its own
                // window (Win+V parity) — the cat never moves. Linux/Windows-
                // portable keep the embedded cat-window panel for the hotkey.
                #[cfg(target_os = "macos")]
                self.open_flyout(event_loop);
                #[cfg(not(target_os = "macos"))]
                {
                    // the hotkey always *shows* the panel (never toggles
                    // closed) and reveals the window so an obscured or hidden
                    // panel reappears
                    self.pet.open_panel();
                    self.reveal();
                }
            }
            // the macOS event tap couldn't start (Accessibility not granted)
            if self.perm_needed.swap(false, Ordering::Relaxed) {
                self.pet.notify_accessibility_needed();
            }
            // the daily release check found a newer version
            if let Some(v) = crate::update::take_found() {
                self.pet.notify_update(&v);
            }
            // copy events observed by the clipboard watcher
            self.capture_flag
                .store(self.pet.st.clip_capture, Ordering::Relaxed);
            while let Ok(cap) = self.clip_rx.try_recv() {
                self.pet.on_copy_rich(cap.text, None, None, cap.formats);
            }
            let (k, c, wh) = input::drain();
            let redraw = self.pet.advance(k, c, wh);
            let _ = self.pet.take_level_changed(); // no tray to update here
            if self.pet.take_size_changed() {
                self.apply_size();
            }
            if self.pet.should_autosave() {
                self.save_position();
            }
            if redraw {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            // the caret-anchored flyout paints into its own window; a grip
            // resize rebuilds its buffers
            #[cfg(target_os = "macos")]
            if redraw && self.pet.flyout_open() {
                if self.pet.take_flyout_resized() {
                    self.resize_flyout_buffers();
                }
                if let Some(f) = self.flyout.as_ref() {
                    f.window.request_redraw();
                }
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.last_frame + TICK));
    }
}

/// Maps a winit key event (+ Ctrl/Cmd state) to a panel [`NavKey`]. Shared by
/// the embedded panel ([`PortableApp::panel_key`]) and the flyout window.
fn nav_from_key(event: &winit::event::KeyEvent, ctrl: bool) -> Option<NavKey> {
    match event.physical_key {
        PhysicalKey::Code(KeyCode::ArrowUp) => Some(NavKey::Up),
        PhysicalKey::Code(KeyCode::ArrowDown) => Some(NavKey::Down),
        PhysicalKey::Code(KeyCode::PageUp) => Some(NavKey::PageUp),
        PhysicalKey::Code(KeyCode::PageDown) => Some(NavKey::PageDown),
        PhysicalKey::Code(KeyCode::Home) => Some(NavKey::Home),
        PhysicalKey::Code(KeyCode::End) => Some(NavKey::End),
        PhysicalKey::Code(KeyCode::ArrowRight) => Some(NavKey::Right),
        PhysicalKey::Code(KeyCode::ArrowLeft) => Some(NavKey::Left),
        // Ctrl/Cmd+Enter: paste the selection as plain text (formatting stripped)
        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) if ctrl => Some(NavKey::PasteText),
        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Some(NavKey::Enter),
        PhysicalKey::Code(KeyCode::Delete) => Some(NavKey::Delete),
        PhysicalKey::Code(KeyCode::Backspace) => Some(NavKey::Backspace),
        PhysicalKey::Code(KeyCode::Escape) => Some(NavKey::Esc),
        PhysicalKey::Code(KeyCode::Tab) => Some(NavKey::Tab), // source filter
        PhysicalKey::Code(KeyCode::KeyP) if ctrl => Some(NavKey::Pin),
        PhysicalKey::Code(KeyCode::KeyZ) if ctrl => Some(NavKey::Undo),
        // Ctrl/Cmd+0..9: quick-copy the nth row from the top
        PhysicalKey::Code(code) if ctrl => quick_digit(code).map(NavKey::Quick),
        _ => None,
    }
}

/// Ctrl/Cmd+digit → quick-copy slot (0 = the top clip).
fn quick_digit(code: KeyCode) -> Option<u8> {
    Some(match code {
        KeyCode::Digit0 | KeyCode::Numpad0 => 0,
        KeyCode::Digit1 | KeyCode::Numpad1 => 1,
        KeyCode::Digit2 | KeyCode::Numpad2 => 2,
        KeyCode::Digit3 | KeyCode::Numpad3 => 3,
        KeyCode::Digit4 | KeyCode::Numpad4 => 4,
        KeyCode::Digit5 | KeyCode::Numpad5 => 5,
        KeyCode::Digit6 | KeyCode::Numpad6 => 6,
        KeyCode::Digit7 | KeyCode::Numpad7 => 7,
        KeyCode::Digit8 | KeyCode::Numpad8 => 8,
        KeyCode::Digit9 | KeyCode::Numpad9 => 9,
        _ => return None,
    })
}

/// The rdev key the configured hotkey letter/digit corresponds to.
fn rdev_key_of(c: char) -> Option<rdev::Key> {
    use rdev::Key::*;
    Some(match c {
        'A' => KeyA, 'B' => KeyB, 'C' => KeyC, 'D' => KeyD, 'E' => KeyE,
        'F' => KeyF, 'G' => KeyG, 'H' => KeyH, 'I' => KeyI, 'J' => KeyJ,
        'K' => KeyK, 'L' => KeyL, 'M' => KeyM, 'N' => KeyN, 'O' => KeyO,
        'P' => KeyP, 'Q' => KeyQ, 'R' => KeyR, 'S' => KeyS, 'T' => KeyT,
        'U' => KeyU, 'V' => KeyV, 'W' => KeyW, 'X' => KeyX, 'Y' => KeyY,
        'Z' => KeyZ,
        '0' => Num0, '1' => Num1, '2' => Num2, '3' => Num3, '4' => Num4,
        '5' => Num5, '6' => Num6, '7' => Num7, '8' => Num8, '9' => Num9,
        _ => return None,
    })
}

/// Matches the global key stream against one configured chord. Holds only
/// five booleans of state (the four modifiers + main-key-down for repeat
/// suppression); key identities are compared and immediately discarded —
/// never stored, logged or transmitted (ADR-0008).
struct ChordTracker {
    hk: Hotkey,
    main: Option<rdev::Key>,
    ctrl: bool,
    shift: bool,
    alt: bool,
    meta: bool,
    main_down: bool,
}

impl ChordTracker {
    fn new(hk: Hotkey) -> ChordTracker {
        ChordTracker {
            main: rdev_key_of(hk.key),
            hk,
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
            main_down: false,
        }
    }

    /// Swaps the chord this tracker matches (the user cycled the panel hotkey).
    /// Refreshes the cached `main` key so the new chord is recognised.
    fn set_hotkey(&mut self, hk: Hotkey) {
        self.main = rdev_key_of(hk.key);
        self.hk = hk;
    }

    /// Feeds one global key event; true when the chord fired (on the initial
    /// press only — OS auto-repeat while held does not re-fire).
    fn on_event(&mut self, event: &rdev::EventType) -> bool {
        use rdev::Key::*;
        let (key, down) = match event {
            rdev::EventType::KeyPress(k) => (k, true),
            rdev::EventType::KeyRelease(k) => (k, false),
            _ => return false,
        };
        match key {
            ControlLeft | ControlRight => self.ctrl = down,
            ShiftLeft | ShiftRight => self.shift = down,
            Alt | AltGr => self.alt = down,
            MetaLeft | MetaRight => self.meta = down,
            k if Some(*k) == self.main => {
                let fresh = down && !self.main_down;
                self.main_down = down;
                return fresh
                    && self.meta == self.hk.win
                    && self.ctrl == self.hk.ctrl
                    && self.shift == self.hk.shift
                    && self.alt == self.hk.alt;
            }
            _ => {}
        }
        false
    }
}

/// Synthesizes the paste shortcut (Ctrl+V, or Cmd+V on macOS) into the focused
/// app via `rdev::simulate` (auto-paste). Output-only: it injects keystrokes,
/// never reads them, so the input-privacy guarantee is untouched (golden rule
/// 1); and being a `simulate` call it avoids the macOS TIS *listen* crash path
/// (LNR-0005).
fn paste_synthesize() {
    #[cfg(target_os = "macos")]
    let modifier = rdev::Key::MetaLeft;
    #[cfg(not(target_os = "macos"))]
    let modifier = rdev::Key::ControlLeft;
    for et in [
        rdev::EventType::KeyPress(modifier),
        rdev::EventType::KeyPress(rdev::Key::KeyV),
        rdev::EventType::KeyRelease(rdev::Key::KeyV),
        rdev::EventType::KeyRelease(modifier),
    ] {
        let _ = rdev::simulate(&et);
    }
}

/// Suppresses OS key auto-repeat for the activity counters: a key counts
/// once when pressed and not again until released. Key identities are kept
/// only while the key is physically held, then dropped — never logged,
/// persisted or transmitted (ADR-0008 / golden rule 1). On X11, auto-repeat
/// arrives as release+press pairs and passes this gate; macOS (repeated
/// KeyDown without KeyUp) and Windows-portable are fully filtered.
struct KeyGate {
    down: Vec<rdev::Key>,
}

impl KeyGate {
    fn new() -> KeyGate {
        KeyGate { down: Vec::new() }
    }

    /// True when this press is fresh (the key was not already held).
    fn fresh_press(&mut self, k: rdev::Key) -> bool {
        if self.down.contains(&k) {
            return false;
        }
        // a missed release can't wedge the gate: beyond this many "held"
        // keys the press still counts, it just isn't tracked
        if self.down.len() < 64 {
            self.down.push(k);
        }
        true
    }

    fn release(&mut self, k: rdev::Key) {
        self.down.retain(|d| *d != k);
    }
}

/// One global input event: bump the shared activity counters (key presses
/// gated for auto-repeat) and feed the chord matcher (the only key
/// inspection we permit; see [`ChordTracker`] / [`KeyGate`]).
///
/// `hk_rx` carries runtime hotkey changes (the user cycled the preset); we
/// drain it with a cheap non-blocking `try_recv` so the swap happens off the
/// hot path without locking on every keystroke.
fn pump(
    et: rdev::EventType,
    chord: &mut ChordTracker,
    gate: &mut KeyGate,
    panel_toggle: &AtomicBool,
    hk_rx: &Receiver<Hotkey>,
) {
    while let Ok(hk) = hk_rx.try_recv() {
        chord.set_hotkey(hk);
    }
    match et {
        rdev::EventType::KeyPress(k) => {
            if gate.fresh_press(k) {
                input::key();
            }
        }
        rdev::EventType::KeyRelease(k) => gate.release(k),
        rdev::EventType::ButtonPress(_) => input::click(),
        rdev::EventType::Wheel { .. } => input::wheel(),
        _ => {}
    }
    if chord.on_event(&et) {
        panel_toggle.store(true, Ordering::Relaxed);
    }
}

/// Spawns the global input listener: increments the shared counters on every
/// key/button/wheel event system-wide and raises `panel_toggle` when the
/// configured panel chord is pressed.
///
/// macOS uses [`super::mac_input`] (a TIS-free event tap; LNR-0005); if its
/// tap can't be installed — Accessibility permission not granted —
/// `perm_needed` is raised and the listener exits without crashing the app.
/// Other platforms use `rdev::listen`.
/// `hk_rx` receives runtime hotkey changes (preset cycling) and is drained
/// inside [`pump`].
#[cfg(target_os = "macos")]
fn spawn_global_input(
    hk: Hotkey,
    hk_rx: Receiver<Hotkey>,
    panel_toggle: Arc<AtomicBool>,
    perm_needed: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut chord = ChordTracker::new(hk);
        let mut gate = KeyGate::new();
        let res = super::mac_input::listen(move |et| {
            pump(et, &mut chord, &mut gate, &panel_toggle, &hk_rx)
        });
        if res.is_err() {
            perm_needed.store(true, Ordering::Relaxed);
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn spawn_global_input(
    hk: Hotkey,
    hk_rx: Receiver<Hotkey>,
    panel_toggle: Arc<AtomicBool>,
    _perm_needed: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut chord = ChordTracker::new(hk);
        let mut gate = KeyGate::new();
        let _ = rdev::listen(move |event| {
            pump(event.event_type, &mut chord, &mut gate, &panel_toggle, &hk_rx)
        });
    });
}

/// Spawns the clipboard watcher: polls `arboard` for text changes and sends
/// new copies down the channel. Skips (once) whatever we set ourselves via
/// the shared `suppress` marker, ignores whatever was already on the
/// clipboard at startup, and — while capture is paused — doesn't read the
/// clipboard at all (resyncing silently on resume so paused-time copies are
/// never retroactively captured).
fn spawn_clipboard_watcher(tx: Sender<ClipCapture>, suppress: Suppress, capture: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let Ok(mut cb) = arboard::Clipboard::new() else {
            return; // no clipboard (e.g. pure Wayland without XWayland)
        };
        let mut last: Option<String> = cb.get_text().ok();
        let mut paused = false;
        loop {
            std::thread::sleep(CLIP_POLL);
            if !capture.load(Ordering::Relaxed) {
                paused = true;
                continue;
            }
            let Ok(text) = cb.get_text() else { continue };
            if std::mem::take(&mut paused) {
                last = Some(text); // resume: resync without emitting
                continue;
            }
            if last.as_deref() == Some(text.as_str()) {
                continue;
            }
            last = Some(text.clone());
            // our own copy-back? consume the marker and skip one event
            if let Ok(mut guard) = suppress.lock() {
                if guard.as_deref() == Some(text.as_str()) {
                    *guard = None;
                    continue;
                }
            }
            // arboard already told us the text changed; on macOS also grab the
            // original rich formats for the same content (Linux stays plain).
            #[cfg(target_os = "macos")]
            let formats = super::mac_clipboard::read_formats();
            #[cfg(not(target_os = "macos"))]
            let formats = None;
            if tx.send(ClipCapture { text, formats }).is_err() {
                return;
            }
        }
    });
}

pub fn run() {
    #[cfg(windows)]
    crate::sound::set_player(super::windows_sound::play);
    crate::sound::init();

    let (tx, rx) = std::sync::mpsc::channel();
    let suppress: Suppress = Arc::new(Mutex::new(None));
    let st = Persist::load();
    let capture_flag = Arc::new(AtomicBool::new(st.clip_capture));
    spawn_clipboard_watcher(tx, suppress.clone(), capture_flag.clone());

    // daily GitHub release check (ADR-0009)
    crate::update::set_enabled(st.auto_update);
    crate::update::spawn_checker();

    let hk = Hotkey::from_spec(&st.hotkey);
    let panel_toggle = Arc::new(AtomicBool::new(false));
    let perm_needed = Arc::new(AtomicBool::new(false));
    // Runtime hotkey changes (preset cycling) flow to the listener thread here.
    let (hk_tx, hk_rx) = std::sync::mpsc::channel::<Hotkey>();
    spawn_global_input(hk, hk_rx, panel_toggle.clone(), perm_needed.clone());

    let mut pet = Pet::new(st);
    pet.set_panel_hint(format!("{} / C", hk.display()));
    let signals = InputSignals {
        panel_toggle,
        perm_needed,
        hk_tx,
    };
    let mut app = PortableApp::new(pet, rx, suppress, capture_flag, signals, hk.display());

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(_) => return,
    };
    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + TICK));
    let _ = event_loop.run_app(&mut app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rdev::EventType::{KeyPress, KeyRelease};
    use rdev::Key;

    fn tracker(spec: &str) -> ChordTracker {
        ChordTracker::new(Hotkey::from_spec(spec))
    }

    #[test]
    fn fires_on_exact_chord_only() {
        let mut t = tracker("win+shift+v"); // = Cmd+Shift+V on macOS
        assert!(!t.on_event(&KeyPress(Key::MetaLeft)));
        assert!(!t.on_event(&KeyPress(Key::ShiftLeft)));
        assert!(t.on_event(&KeyPress(Key::KeyV)), "chord complete");
        // wrong modifiers never fire: Ctrl+Shift+V is a different chord
        let mut t = tracker("win+shift+v");
        t.on_event(&KeyPress(Key::ControlLeft));
        t.on_event(&KeyPress(Key::ShiftLeft));
        assert!(!t.on_event(&KeyPress(Key::KeyV)));
        // extra modifier on top of the chord also blocks it
        let mut t = tracker("win+shift+v");
        t.on_event(&KeyPress(Key::MetaLeft));
        t.on_event(&KeyPress(Key::ShiftLeft));
        t.on_event(&KeyPress(Key::ControlLeft));
        assert!(!t.on_event(&KeyPress(Key::KeyV)));
    }

    #[test]
    fn auto_repeat_fires_once_until_released() {
        let mut t = tracker("win+shift+v");
        t.on_event(&KeyPress(Key::MetaRight));
        t.on_event(&KeyPress(Key::ShiftRight));
        assert!(t.on_event(&KeyPress(Key::KeyV)));
        assert!(!t.on_event(&KeyPress(Key::KeyV)), "OS auto-repeat");
        assert!(!t.on_event(&KeyRelease(Key::KeyV)));
        assert!(t.on_event(&KeyPress(Key::KeyV)), "re-press fires again");
    }

    #[test]
    fn releasing_a_modifier_disarms() {
        let mut t = tracker("ctrl+alt+9");
        t.on_event(&KeyPress(Key::ControlLeft));
        t.on_event(&KeyPress(Key::Alt));
        t.on_event(&KeyRelease(Key::Alt));
        assert!(!t.on_event(&KeyPress(Key::Num9)));
        t.on_event(&KeyPress(Key::AltGr)); // right alt counts too
        t.on_event(&KeyRelease(Key::Num9));
        assert!(t.on_event(&KeyPress(Key::Num9)));
    }

    #[test]
    fn set_hotkey_swaps_the_matched_chord_at_runtime() {
        // start matching Win+Shift+V
        let mut t = tracker("win+shift+v");
        t.on_event(&KeyPress(Key::MetaLeft));
        t.on_event(&KeyPress(Key::ShiftLeft));
        assert!(t.on_event(&KeyPress(Key::KeyV)), "old chord fires");
        // user lifts the keys, then cycles the preset to Ctrl+Shift+C
        t.on_event(&KeyRelease(Key::KeyV));
        t.on_event(&KeyRelease(Key::ShiftLeft));
        t.on_event(&KeyRelease(Key::MetaLeft));
        t.set_hotkey(Hotkey::from_spec("ctrl+shift+c"));
        // the new combo must now fire
        t.on_event(&KeyPress(Key::ControlLeft));
        t.on_event(&KeyPress(Key::ShiftLeft));
        assert!(t.on_event(&KeyPress(Key::KeyC)), "new chord fires after swap");
        // the previous chord no longer triggers
        let mut t = tracker("win+shift+v");
        t.set_hotkey(Hotkey::from_spec("ctrl+shift+c"));
        t.on_event(&KeyPress(Key::MetaLeft));
        t.on_event(&KeyPress(Key::ShiftLeft));
        assert!(!t.on_event(&KeyPress(Key::KeyV)), "old chord is disarmed");
    }

    #[test]
    fn plain_key_without_modifiers_never_fires() {
        let mut t = tracker("win+shift+v");
        assert!(!t.on_event(&KeyPress(Key::KeyV)));
        // and non-key events are ignored entirely
        assert!(!t.on_event(&rdev::EventType::Wheel { delta_x: 0, delta_y: 1 }));
    }

    #[test]
    fn key_gate_ignores_auto_repeat_until_release() {
        let mut g = KeyGate::new();
        assert!(g.fresh_press(Key::KeyA));
        assert!(!g.fresh_press(Key::KeyA), "auto-repeat while held");
        assert!(g.fresh_press(Key::KeyB), "other keys still count");
        g.release(Key::KeyA);
        assert!(g.fresh_press(Key::KeyA), "re-press after release counts");
        // releasing a key that was never tracked is harmless
        g.release(Key::KeyZ);
        assert!(!g.fresh_press(Key::KeyB), "B is still held");
    }
}
