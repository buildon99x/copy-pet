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
//! key). Settings have no system tray here; when the window is focused
//! these keys apply: S size · A accessory · M sound · B stats bubble ·
//! L lock · G language · C clipboard panel · Q/Esc quit. While the panel is
//! open the keyboard drives it instead (type to search, arrows/enter, Tab
//! cycles the source-app filter, Esc closes).
//!
//! Privacy note (ADR-0008): the rdev listener increments the activity
//! counters and additionally compares each key event against the one
//! configured hotkey chord, in memory, discarding it immediately — key
//! identities are never stored, logged, buffered or transmitted.

use crate::hotkey::Hotkey;
use crate::input;
use crate::panel::NavKey;
use crate::pet::Pet;
use crate::state::{Persist, ACCESSORIES};
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
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId, WindowLevel};

const TICK: Duration = Duration::from_millis(33);
const DBLCLICK: Duration = Duration::from_millis(350);
/// Clipboard poll cadence (arboard has no change notifications).
const CLIP_POLL: Duration = Duration::from_millis(400);

type Surface = softbuffer::Surface<Rc<Window>, Rc<Window>>;
/// Text we just wrote to the clipboard ourselves; the watcher skips it once.
type Suppress = Arc<Mutex<Option<String>>>;

struct PortableApp {
    pet: Pet,
    window: Option<Rc<Window>>,
    _context: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<Surface>,
    pm: tiny_skia::Pixmap,
    w: u32,
    h: u32,
    last_frame: Instant,
    // clipboard
    clip_rx: Receiver<String>,
    suppress: Suppress,
    /// Mirror of `st.clip_capture` for the watcher thread: while false the
    /// clipboard is not even read.
    capture_flag: Arc<AtomicBool>,
    /// Raised by the global-input thread when the panel hotkey chord fires.
    panel_toggle: Arc<AtomicBool>,
    // interaction
    cursor: PhysicalPosition<f64>,
    mouse_down: bool,
    dragging: bool,
    press_pos: PhysicalPosition<f64>,
    last_click: Option<Instant>,
    focused: bool,
}

impl PortableApp {
    fn new(
        pet: Pet,
        clip_rx: Receiver<String>,
        suppress: Suppress,
        capture_flag: Arc<AtomicBool>,
        panel_toggle: Arc<AtomicBool>,
    ) -> Self {
        let (w, h) = pet.canvas_size();
        PortableApp {
            pet,
            window: None,
            _context: None,
            surface: None,
            pm: tiny_skia::Pixmap::new(w as u32, h as u32).unwrap(),
            w: w as u32,
            h: h as u32,
            last_frame: Instant::now(),
            clip_rx,
            suppress,
            capture_flag,
            panel_toggle,
            cursor: PhysicalPosition::new(0.0, 0.0),
            mouse_down: false,
            dragging: false,
            press_pos: PhysicalPosition::new(0.0, 0.0),
            last_click: None,
            focused: false,
        }
    }

    /// Resizes the window/buffers to the wanted size (scale or panel state
    /// changed), keeping the bottom edge roughly anchored, then repaints.
    fn apply_size(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let (nw, nh) = self.pet.canvas_size();
        let (nw, nh) = (nw as u32, nh as u32);

        // anchor bottom-center
        if let Ok(pos) = window.outer_position() {
            let nx = pos.x + (self.w as i32 - nw as i32) / 2;
            let ny = pos.y + (self.h as i32 - nh as i32);
            window.set_outer_position(PhysicalPosition::new(nx, ny));
        }
        let _ = window.request_inner_size(PhysicalSize::new(nw, nh));
        self.w = nw;
        self.h = nh;
        self.pm = tiny_skia::Pixmap::new(nw, nh).unwrap();
        self.resize_surface();
        self.paint();
    }

    fn resize_surface(&mut self) {
        if let (Some(surface), Some(w), Some(h)) =
            (self.surface.as_mut(), NonZeroU32::new(self.w), NonZeroU32::new(self.h))
        {
            let _ = surface.resize(w, h);
        }
    }

    /// Renders the pet into the pixmap and presents it via softbuffer.
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

    fn canvas_xy(&self) -> (f32, f32) {
        let s = self.pet.scale();
        (self.cursor.x as f32 / s, self.cursor.y as f32 / s)
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

    /// Puts text on the OS clipboard (a clip picked from the panel).
    fn set_clipboard(&self, text: String) {
        if let Ok(mut guard) = self.suppress.lock() {
            *guard = Some(text.clone());
        }
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(text);
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
            KeyCode::KeyQ | KeyCode::Escape => {
                self.save_position();
                event_loop.exit();
            }
            _ => {}
        }
    }

    /// Keyboard input while the panel is open: search + navigation.
    fn panel_key(&mut self, event: &winit::event::KeyEvent) {
        let nav = match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => Some(NavKey::Up),
            PhysicalKey::Code(KeyCode::ArrowDown) => Some(NavKey::Down),
            PhysicalKey::Code(KeyCode::PageUp) => Some(NavKey::PageUp),
            PhysicalKey::Code(KeyCode::PageDown) => Some(NavKey::PageDown),
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Some(NavKey::Enter),
            PhysicalKey::Code(KeyCode::Delete) => Some(NavKey::Delete),
            PhysicalKey::Code(KeyCode::Backspace) => Some(NavKey::Backspace),
            PhysicalKey::Code(KeyCode::Escape) => Some(NavKey::Esc),
            PhysicalKey::Code(KeyCode::Tab) => Some(NavKey::Tab), // source filter
            _ => None,
        };
        if let Some(key) = nav {
            if let Some(text) = self.pet.panel_nav(key) {
                self.set_clipboard(text);
            }
            return;
        }
        if let Some(txt) = &event.text {
            for c in txt.chars() {
                self.pet.panel_char(c);
            }
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

        let attrs = Window::default_attributes()
            .with_title("ClipCat")
            .with_inner_size(PhysicalSize::new(w as u32, h as u32))
            .with_position(pos)
            .with_decorations(false)
            .with_transparent(false)
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

        let context = softbuffer::Context::new(window.clone()).ok();
        let surface = context
            .as_ref()
            .and_then(|ctx| softbuffer::Surface::new(ctx, window.clone()).ok());

        self.window = Some(window);
        self._context = context;
        self.surface = surface;
        self.resize_surface();
        self.last_frame = Instant::now();
        self.paint();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
                let (cx, cy) = self.canvas_xy();
                self.pet.set_cursor(cx, cy);
                if self.mouse_down && !self.dragging && !self.pet.st.locked {
                    let dx = position.x - self.press_pos.x;
                    let dy = position.y - self.press_pos.y;
                    if dx * dx + dy * dy > 9.0 {
                        self.dragging = true;
                        self.mouse_down = false;
                        if let Some(window) = &self.window {
                            let _ = window.drag_window();
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Left => match state {
                    ElementState::Pressed => {
                        let (cx, cy) = self.canvas_xy();
                        if self.pet.panel_hit(cx, cy) {
                            // panel interactions act on press; no drag/petting
                            if let Some(text) = self.pet.panel_click(cx, cy) {
                                self.set_clipboard(text);
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
                        if self.mouse_down && !self.dragging {
                            let (cx, cy) = self.canvas_xy();
                            let (lx, ly) = self.pet.cat_point(cx, cy);
                            self.pet.click_bounce(lx, ly);
                        }
                        self.mouse_down = false;
                        self.dragging = false;
                    }
                },
                MouseButton::Middle => {
                    if state == ElementState::Pressed {
                        self.pet.toggle_panel();
                    }
                }
                _ => {}
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                let (cx, cy) = self.canvas_xy();
                if self.pet.panel_hit(cx, cy) {
                    if dy != 0.0 {
                        self.pet.panel_wheel(if dy < 0.0 { 1 } else { -1 });
                    }
                } else if dy != 0.0 {
                    // a local scroll over the pet still counts as activity
                    input::wheel();
                }
            }
            WindowEvent::Ime(Ime::Commit(s)) => {
                if self.pet.panel_open() {
                    for c in s.chars() {
                        self.pet.panel_char(c);
                    }
                }
            }
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
                self.pet.toggle_panel();
            }
            // copy events observed by the clipboard watcher
            self.capture_flag
                .store(self.pet.st.clip_capture, Ordering::Relaxed);
            while let Ok(text) = self.clip_rx.try_recv() {
                self.pet.on_copy(text, None, None);
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
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.last_frame + TICK));
    }
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

/// Spawns the `rdev` global input listener: increments the shared counters
/// on every key/button/wheel event system-wide and raises `panel_toggle`
/// when the configured panel chord is pressed (see [`ChordTracker`] for the
/// privacy boundary).
fn spawn_global_input(hk: Hotkey, panel_toggle: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut chord = ChordTracker::new(hk);
        let _ = rdev::listen(move |event| {
            match event.event_type {
                rdev::EventType::KeyPress(_) => input::key(),
                rdev::EventType::ButtonPress(_) => input::click(),
                rdev::EventType::Wheel { .. } => input::wheel(),
                _ => {}
            }
            if chord.on_event(&event.event_type) {
                panel_toggle.store(true, Ordering::Relaxed);
            }
        });
    });
}

/// Spawns the clipboard watcher: polls `arboard` for text changes and sends
/// new copies down the channel. Skips (once) whatever we set ourselves via
/// the shared `suppress` marker, ignores whatever was already on the
/// clipboard at startup, and — while capture is paused — doesn't read the
/// clipboard at all (resyncing silently on resume so paused-time copies are
/// never retroactively captured).
fn spawn_clipboard_watcher(tx: Sender<String>, suppress: Suppress, capture: Arc<AtomicBool>) {
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
            if tx.send(text).is_err() {
                return;
            }
        }
    });
}

pub fn run() {
    crate::sound::init();

    let (tx, rx) = std::sync::mpsc::channel();
    let suppress: Suppress = Arc::new(Mutex::new(None));
    let st = Persist::load();
    let capture_flag = Arc::new(AtomicBool::new(st.clip_capture));
    spawn_clipboard_watcher(tx, suppress.clone(), capture_flag.clone());

    let hk = Hotkey::from_spec(&st.hotkey);
    let panel_toggle = Arc::new(AtomicBool::new(false));
    spawn_global_input(hk, panel_toggle.clone());

    let mut pet = Pet::new(st);
    pet.set_panel_hint(format!("{} / C", hk.display()));
    let mut app = PortableApp::new(pet, rx, suppress, capture_flag, panel_toggle);

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
    fn plain_key_without_modifiers_never_fires() {
        let mut t = tracker("win+shift+v");
        assert!(!t.on_event(&KeyPress(Key::KeyV)));
        // and non-key events are ignored entirely
        assert!(!t.on_event(&rdev::EventType::Wheel { delta_x: 0, delta_y: 1 }));
    }
}
