//! Portable backend for macOS, Linux and (with `--features portable`) Windows.
//!
//! Windowing via `winit`, software presentation via `softbuffer`, and global
//! keyboard/mouse activity via `rdev` running on its own listener thread. The
//! pet is drawn on an opaque rounded "card" because `softbuffer` cannot carry
//! per-pixel alpha to the desktop compositor (the native Win32 backend keeps
//! the fully transparent, click-through look). See ADR-0001 / ADR-0003.
//!
//! Interactions: drag to move, double-click to pet, single-click to bounce,
//! hover to show today's stats. Settings have no system tray here; when the
//! window is focused these keys apply (also shown via `--help`-style tooltip):
//!   S size · A accessory · M sound · B stats bubble · L lock · Q/Esc quit.

use crate::input;
use crate::pet::{window_size, Pet};
use crate::state::{Persist, ACCESSORIES};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId, WindowLevel};

const TICK: Duration = Duration::from_millis(33);
const DBLCLICK: Duration = Duration::from_millis(350);

type Surface = softbuffer::Surface<Rc<Window>, Rc<Window>>;

struct PortableApp {
    pet: Pet,
    window: Option<Rc<Window>>,
    _context: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<Surface>,
    pm: tiny_skia::Pixmap,
    w: u32,
    h: u32,
    last_frame: Instant,
    // interaction
    cursor: PhysicalPosition<f64>,
    mouse_down: bool,
    dragging: bool,
    press_pos: PhysicalPosition<f64>,
    last_click: Option<Instant>,
    focused: bool,
}

impl PortableApp {
    fn new(pet: Pet) -> Self {
        let (w, h) = window_size(pet.scale());
        PortableApp {
            pet,
            window: None,
            _context: None,
            surface: None,
            pm: tiny_skia::Pixmap::new(w as u32, h as u32).unwrap(),
            w: w as u32,
            h: h as u32,
            last_frame: Instant::now(),
            cursor: PhysicalPosition::new(0.0, 0.0),
            mouse_down: false,
            dragging: false,
            press_pos: PhysicalPosition::new(0.0, 0.0),
            last_click: None,
            focused: false,
        }
    }

    /// Resizes the window/buffers to the current scale, keeping the bottom
    /// edge roughly anchored, then repaints.
    fn apply_scale(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let (nw, nh) = window_size(self.pet.scale());
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

    /// Applies a keyboard shortcut. Returns true if it requested quit.
    fn shortcut(&mut self, code: KeyCode, event_loop: &ActiveEventLoop) {
        match code {
            KeyCode::KeyS => {
                self.pet.st.scale_idx = (self.pet.st.scale_idx + 1) % 3;
                self.pet.dirty = true;
                self.apply_scale();
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
            KeyCode::KeyQ | KeyCode::Escape => {
                self.save_position();
                event_loop.exit();
            }
            _ => {}
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
            .with_title("DeskCat")
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
            WindowEvent::CursorLeft { .. } => self.pet.set_hover(false),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
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
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
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
                                self.pet.click_bounce(cx, cy);
                            }
                            self.mouse_down = false;
                            self.dragging = false;
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // a local scroll over the pet still counts as activity
                let moved = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y != 0.0,
                    MouseScrollDelta::PixelDelta(p) => p.y != 0.0,
                };
                if moved {
                    input::wheel();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(code) = event.physical_key {
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
            let (k, c, wh) = input::drain();
            let redraw = self.pet.advance(k, c, wh);
            let _ = self.pet.take_level_changed(); // no tray to update here
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

/// Spawns the `rdev` global input listener. Increments the shared counters on
/// every key/button/wheel event system-wide; reads no key contents.
fn spawn_global_input() {
    std::thread::spawn(|| {
        let _ = rdev::listen(|event| match event.event_type {
            rdev::EventType::KeyPress(_) => input::key(),
            rdev::EventType::ButtonPress(_) => input::click(),
            rdev::EventType::Wheel { .. } => input::wheel(),
            _ => {}
        });
    });
}

pub fn run() {
    crate::sound::init();
    spawn_global_input();

    let st = Persist::load();
    let mut app = PortableApp::new(Pet::new(st));

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(_) => return,
    };
    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + TICK));
    let _ = event_loop.run_app(&mut app);
}
