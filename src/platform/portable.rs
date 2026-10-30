//! Portable winit backend with registered OS shortcuts (ADR-0020).
//! Clipboard observation is independent of shortcut registration. rdev is used
//! only to synthesize optional paste output; no keyboard or mouse monitoring.
//! macOS presents transparent CALayers; Linux/Windows portable use softbuffer.
//! Middle click and local C open history even if global registration fails.

use crate::clipboard::RichFormats;
use crate::hotkey::Hotkey;
use super::registered_hotkey::{HotkeyEvent, RegisteredHotkey};
use crate::hotkey::Registration;
use crate::panel::{self, fit_delta, NavKey, Rect};
use crate::pet::{ClipPick, Pet};
use crate::viewport::Density;

/// A clipboard observation handed from the watcher thread to the main loop:
/// the plain text plus any rich formats the platform could read (macOS only;
/// Linux/arboard is text-only, so `formats` is always `None` there).
struct ClipCapture {
    text: String,
    formats: Option<RichFormats>,
}
use crate::state::{Persist};
#[cfg(not(target_os = "macos"))]
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
#[cfg(target_os = "macos")]
use winit::dpi::LogicalSize;

use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId, WindowLevel};

fn window_density(window: &Window) -> Density {
    #[cfg(target_os = "macos")]
    { Density::new(window.scale_factor()) }
    #[cfg(not(target_os = "macos"))]
    { let _ = window; Density::new(1.0) }
}
const TICK: Duration = Duration::from_millis(33);
/// Clipboard poll cadence (arboard has no change notifications).
const CLIP_POLL: Duration = Duration::from_millis(400);

#[cfg(not(target_os = "macos"))]
type Surface = softbuffer::Surface<Rc<Window>, Rc<Window>>;
/// Text we just wrote to the clipboard ourselves; the watcher skips it once.
type Suppress = Arc<Mutex<Option<String>>>;

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
    density: Density,
    w: u32,
    h: u32,
    last_frame: Instant,
    // clipboard
    clip_rx: Receiver<ClipCapture>,
    suppress: Suppress,
    /// Mirror of `st.clip_capture` for the watcher thread: while false the
    /// clipboard is not even read.
    capture_flag: Arc<AtomicBool>,
    hotkey: RegisteredHotkey,
    hotkey_label: String,
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
    density: Density,
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
fn card_rect(origin_x: f32, origin_y: f32, l: &panel::Layout, density: Density) -> Rect {
    density.screen_rect((origin_x, origin_y), Rect {
        x: l.card_x,
        y: l.card_y,
        w: l.card_w,
        h: l.card_h,
    })
}

impl PortableApp {
    fn new(
        pet: Pet,
        clip_rx: Receiver<ClipCapture>,
        suppress: Suppress,
        capture_flag: Arc<AtomicBool>,
        hotkey: RegisteredHotkey,
        hotkey_label: String,
    ) -> Self {
        let (w, h) = pet.canvas_size();
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
            density: Density::new(1.0),
            w: w as u32,
            h: h as u32,
            last_frame: Instant::now(),
            clip_rx,
            suppress,
            capture_flag,
            hotkey,
            hotkey_label,
            cursor: PhysicalPosition::new(0.0, 0.0),
            mouse_down: false,
            dragging: false,
            panel_dragging: false,
            cat_drag: false,
            drag_screen: (0.0, 0.0),
            press_pos: PhysicalPosition::new(0.0, 0.0),
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
        let (nw, nh) = self.density.physical_size(self.pet.canvas_size());

        let shift = self.pet.take_window_shift();
        let (dx, dy) = self.density.physical_shift(shift);
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
        // Monitor fit runs in physical screen pixels; return logical core units.
        let l = self.pet.panel.layout();
        let card = card_rect(win.x as f32, win.y as f32, &l, self.density);
        let vis = monitor_rect(window)?;
        let (dx, dy) = fit_delta(card, vis);
        (dx.abs() >= 0.5 || dy.abs() >= 0.5)
            .then_some(self.density.logical((dx as f64, dy as f64)))
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
        self.pet.render_at_density(&mut self.pm, self.density.scale());
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

    /// Convert the raw physical cursor once at the logical core boundary.
    fn client_xy(&self) -> (f32, f32) {
        self.density.logical((self.cursor.x, self.cursor.y))
    }

    /// Clears the pending single/double-click state — used whenever a press
    /// is consumed by something other than the click/petting gesture.
    fn reset_click_state(&mut self) {
        self.mouse_down = false;
        self.pet.cancel_cat_press();
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
        if pick.image.is_some() { return; } // Image clipboard is Windows-native only.
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

    /// Replaces the OS registration and labels only the chord that succeeded.
    fn apply_new_hotkey(&mut self, spec: &str) {
        let mut registration = Registration::default();
        registration.rebind(spec, |hk| self.hotkey.bind(hk));
        self.hotkey_label = registration.label();
        self.pet.set_panel_hint(if self.hotkey_label.is_empty() { "C".into() }
            else { format!("{} / C", self.hotkey_label) });
        if self.hotkey_label.is_empty() { self.pet.notify_hotkey_failed(); }
        else if registration.fallback() {
            self.pet.notify_hotkey_fallback(&Hotkey::from_spec(spec).display(), &self.hotkey_label);
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
    /// `Pet::apply_menu_action`; the outcomes that need OS work (
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
            MenuOutcome::ShowAbout => {
                let body = about_text(
                    lang,
                    env!("CARGO_PKG_VERSION"),
                    &self.hotkey_label,
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
            KeyCode::KeyA => self.pet.open_cat_picker(),
            KeyCode::KeyM => {
                self.pet.st.sound_mode = (self.pet.st.sound_mode + 1) % 2;
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
        if self.paste_workbench_editor(event, ctrl) {
            return;
        }
        if let Some(key) = nav_from_key(event, ctrl) {
            if let Some(pick) = self.pet.panel_nav(key) {
                self.set_clipboard(pick);
            }
            return;
        }
        self.forward_text_input(event, ctrl);
    }

    /// Clipboard reads are explicit and scoped to the workbench text editor.
    fn paste_workbench_editor(&mut self, event: &winit::event::KeyEvent, ctrl: bool) -> bool {
        if !ctrl
            || event.physical_key != PhysicalKey::Code(KeyCode::KeyV)
            || !self.pet.workbench_editing()
        {
            return false;
        }
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                self.pet.workbench_paste(&text);
            }
        }
        true
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
                .with_inner_size(LogicalSize::new(fw, fh))
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
                density: Density::new(1.0),
                w: fw,
                h: fh,
                cursor: PhysicalPosition::new(0.0, 0.0),
                move_drag: false,
                resize_drag: false,
                drag_screen: (0.0, 0.0),
            });
        }
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
            let (fw, fh) = f.density.physical_size((fw as i32, fh as i32));
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
        let Some(f) = self.flyout.as_mut() else {
            return;
        };
        let (ax, ay) = anchor.unwrap_or((40.0, 40.0));
        // window top-left so the card (at card_x/card_y inside it) hits anchor
        let mut wx = ax;
        let mut wy = ay;
        // park there first so current_monitor resolves the anchor's monitor
        f.window
            .set_outer_position(PhysicalPosition::new(wx as i32, wy as i32));
        f.density = Density::new(f.window.current_monitor()
            .map(|m| m.scale_factor()).unwrap_or_else(|| f.window.scale_factor()));
        wx -= l.card_x as f64 * f.density.scale() as f64;
        wy -= l.card_y as f64 * f.density.scale() as f64;
        if let Some(vis) = monitor_rect(&f.window) {
            let card = card_rect(wx as f32, wy as f32, &l, f.density);
            let (dx, dy) = fit_delta(card, vis);
            wx += dx as f64;
            wy += dy as f64;
        }
        f.window
            .set_outer_position(PhysicalPosition::new(wx as i32, wy as i32));
        self.resize_flyout_buffers();
    }

    fn paint_flyout(&mut self) {
        let Some(f) = self.flyout.as_mut() else {
            return;
        };
        self.pet.render_flyout_at_density(&mut f.pm, f.density.scale());
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
            WindowEvent::ScaleFactorChanged { scale_factor, mut inner_size_writer } => {
                if let Some(f) = self.flyout.as_mut() {
                    f.density = Density::new(scale_factor);
                    let (w, h) = f.density.physical_size(self.pet.flyout_size());
                    let _ = inner_size_writer.request_inner_size(PhysicalSize::new(w, h));
                    f.move_drag = false;
                    f.resize_drag = false;
                }
                self.pet.panel_drag_end();
                self.pet.clear_cursor();
                self.resize_flyout_buffers();
                if let Some(f) = self.flyout.as_ref() {
                    if let (Ok(pos), Some(vis)) = (f.window.outer_position(), monitor_rect(&f.window)) {
                        let l = self.pet.panel.layout_standalone();
                        let card = card_rect(pos.x as f32, pos.y as f32, &l, f.density);
                        let (dx, dy) = fit_delta(card, vis);
                        f.window.set_outer_position(PhysicalPosition::new(
                            pos.x + dx.round() as i32, pos.y + dy.round() as i32));
                    }
                }
                self.paint_flyout();
            }
            WindowEvent::Focused(false) | WindowEvent::CloseRequested => self.close_flyout(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(f) = self.flyout.as_mut() {
                    f.cursor = position;
                }
                let density = self.flyout.as_ref().map(|f| f.density).unwrap_or(Density::new(1.0));
                let (cx, cy) = density.logical((position.x, position.y));
                self.pet.set_cursor(cx, cy);
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
                            let (dx, dy) = density.logical((dx, dy));
                            self.pet.panel_drag_update(dx, dy);
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
                        .map(|f| f.density.logical((f.cursor.x, f.cursor.y)))
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
        if self.paste_workbench_editor(event, ctrl) {
            return;
        }
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

impl ApplicationHandler<HotkeyEvent> for PortableApp {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: HotkeyEvent) {
        #[cfg(target_os = "macos")]
        self.open_flyout(event_loop);
        #[cfg(not(target_os = "macos"))]
        { let _ = event_loop; self.pet.open_panel(); self.reveal(); }
        self.apply_size();
        if let Some(window) = &self.window { window.request_redraw(); }
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (w, h) = self.pet.canvas_size();
        let initial_density = event_loop.primary_monitor()
            .map(|m| if cfg!(target_os = "macos") { m.scale_factor() } else { 1.0 }).unwrap_or(1.0);
        let (pw, ph) = Density::new(initial_density).physical_size((w, h));

        // initial position: saved, else bottom-right of the primary monitor
        let pos = if self.pet.st.has_pos {
            PhysicalPosition::new(self.pet.st.pos_x, self.pet.st.pos_y)
        } else {
            let (mw, mh) = event_loop
                .primary_monitor()
                .map(|m| (m.size().width as i32, m.size().height as i32))
                .unwrap_or((1920, 1080));
            PhysicalPosition::new(mw - pw as i32 - 40, mh - ph as i32 - 64)
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

        #[cfg(target_os = "macos")]
        let attrs = attrs.with_inner_size(LogicalSize::new(w, h));
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

        self.density = window_density(&window);
        self.window = Some(window);
        let (w, h) = self.density.physical_size(self.pet.canvas_size());
        self.w = w;
        self.h = h;
        self.pm = tiny_skia::Pixmap::new(w, h).unwrap();
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
            WindowEvent::RedrawRequested => self.paint(),
            #[cfg(target_os = "macos")]
            WindowEvent::ScaleFactorChanged { scale_factor, mut inner_size_writer } => {
                self.density = Density::new(scale_factor);
                let (w, h) = self.density.physical_size(self.pet.canvas_size());
                let _ = inner_size_writer.request_inner_size(PhysicalSize::new(w, h));
                self.panel_dragging = false;
                self.cat_drag = false;
                self.reset_click_state();
                self.pet.panel_drag_end();
                self.pet.clear_cursor();
                self.apply_size();
                if self.pet.embedded_panel_open() {
                    if let Some(window) = self.window.as_ref() {
                        if let Some((dx, dy)) = self.panel_fit_shift(window) {
                            self.pet.shift_panel(dx, dy);
                            self.apply_size();
                        }
                    }
                }
            }
            WindowEvent::Focused(false) => self.reset_click_state(),
            WindowEvent::CursorEntered { .. } => self.pet.set_hover(true),
            WindowEvent::CursorLeft { .. } => {
                self.pet.set_hover(false);
                self.pet.cancel_cat_press();
                self.pet.clear_cursor();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                let (cx, cy) = self.client_xy();
                self.pet.set_cursor(cx, cy);
                self.pet.cat_motion(cx, cy);
                if self.panel_dragging || self.cat_drag {
                    // card drag / cat-body drag (mirror images of each other):
                    // Keep screen deltas raw until crossing into logical core units.
                    let sc = self.screen_cursor();
                    let (dx, dy) = (sc.0 - self.drag_screen.0, sc.1 - self.drag_screen.1);
                    if dx != 0.0 || dy != 0.0 {
                        let (dx, dy) = self.density.logical((dx, dy));
                        if self.panel_dragging {
                            self.pet.panel_drag_update(dx, dy);
                        } else {
                            self.pet.drag_pet(dx, dy);
                        }
                        self.drag_screen = sc;
                    }
                } else if self.mouse_down && !self.dragging && !self.pet.st.locked {
                    let (dx, dy) = self.density.logical((position.x - self.press_pos.x,
                        position.y - self.press_pos.y));
                    if dx * dx + dy * dy > 9.0 {
                        self.pet.cancel_cat_press();
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
                        self.pet.cancel_cat_press();
                        let (cx, cy) = self.client_xy();
                        // header strip / resize grip start a card drag (the
                        // grip pokes slightly past the card edge)
                        if self.pet.panel_drag_start(cx, cy) {
                            self.panel_dragging = true;
                            self.drag_screen = self.screen_cursor();
                            self.reset_click_state();
                            return;
                        }
                        if self.pet.panel_hit(cx, cy) {
                            // panel interactions act on press; no drag/petting
                            if let Some(pick) = self.pet.panel_click(cx, cy) {
                                self.set_clipboard(pick);
                            }
                            self.reset_click_state();
                            return;
                        }
                        self.mouse_down = self.pet.cat_press(cx, cy);
                        self.dragging = false;
                        self.press_pos = self.cursor;
                    }
                    ElementState::Released => {
                        if self.panel_dragging {
                            self.panel_dragging = false;
                            self.pet.panel_drag_end();
                        } else if self.cat_drag {
                            self.cat_drag = false;
                        } else if self.mouse_down && !self.dragging {
                            let (cx, cy) = self.client_xy();
                            self.pet.cat_release(cx, cy);
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
                // Local panel scrolling only.
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
            let redraw = self.pet.advance();
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
        PhysicalKey::Code(KeyCode::KeyA) if ctrl => Some(NavKey::SelectAll),
        PhysicalKey::Code(KeyCode::KeyB) if ctrl => Some(NavKey::Workbench),
        PhysicalKey::Code(KeyCode::KeyT) if ctrl => Some(NavKey::Tags),
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

/// Synthesizes the paste shortcut (Ctrl+V, or Cmd+V on macOS) into the focused
/// app via `rdev::simulate` (auto-paste). Output-only: it injects keystrokes,
/// never reads them, so the input-privacy guarantee is untouched (golden rule
/// 1); and being a `simulate` call it avoids the macOS TIS *listen* crash path
/// (LNR-0005).
fn paste_synthesize() {
    for event in paste_events(cfg!(target_os = "macos")) {
        let _ = rdev::simulate(&event);
    }
}

/// Output-only neutralization prevents still-held panel modifiers from turning
/// a fast pick into paste-special or another shortcut. Never reads input state.
fn paste_events(mac: bool) -> Vec<rdev::EventType> {
    use rdev::{EventType::*, Key::*};
    let mut events: Vec<_> = [ShiftLeft, ShiftRight, ControlLeft, ControlRight, Alt, AltGr]
        .into_iter().map(KeyRelease).collect();
    if mac {
        events.extend([KeyRelease(MetaLeft), KeyRelease(MetaRight)]);
    }
    // Leave Win/Super alone outside macOS: a lone Win-up can open Start.
    let modifier = if mac { MetaLeft } else { ControlLeft };
    events.extend([KeyPress(modifier), KeyPress(KeyV), KeyRelease(KeyV), KeyRelease(modifier)]);
    events
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

    let event_loop = match EventLoop::<HotkeyEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(_) => return,
    };
    let hotkey = RegisteredHotkey::new(event_loop.create_proxy());
    let spec = st.hotkey.clone();
    let pet = Pet::new(st);
    let mut app = PortableApp::new(pet, rx, suppress, capture_flag, hotkey, String::new());
    app.apply_new_hotkey(&spec);
    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + TICK));
    let _ = event_loop.run_app(&mut app);
}

#[cfg(test)]
mod paste_tests {
    use super::*;
    #[test]
    fn synthesized_paste_clears_hotkey_modifiers_before_clean_paste() {
        use rdev::{EventType::*, Key::*};
        for mac in [false, true] {
            let events = paste_events(mac);
            let modifier = if mac { MetaLeft } else { ControlLeft };
            assert_eq!(&events[events.len()-4..], &[KeyPress(modifier), KeyPress(KeyV), KeyRelease(KeyV), KeyRelease(modifier)]);
            for key in [ShiftLeft, ShiftRight, ControlLeft, ControlRight, Alt, AltGr] {
                assert!(events[..events.len()-4].contains(&KeyRelease(key)), "hotkey modifiers cannot alter paste");
            }
            assert!(events[..events.len()-4].iter().all(|event| matches!(event, KeyRelease(_))));
        }
    }
}
