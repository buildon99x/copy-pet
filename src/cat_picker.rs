//! Explicit cat gallery. Static thumbnails and reduced preview frames avoid
//! retaining twelve detailed atlases; decoding is sequential and temporary.
use crate::{
    cat::CatKind,
    hunter_art::CatArt,
    i18n::{self, Lang, PickerText},
    sysfont,
};
use std::time::Instant;
use tiny_skia::{Paint, Pixmap, PixmapPaint, Rect, Transform};

pub const WIDTH: f32 = 600.0;
pub const HEIGHT: f32 = 568.0;
#[derive(Clone, Copy)]
pub enum Action {
    None,
    Apply(CatKind),
    Close,
}
pub struct CatPicker {
    pub selected: CatKind,
    thumbnails: Vec<Pixmap>,
    preview: Vec<Pixmap>,
    mode: u8,
    started: Instant,
    pub previous_size: (f32, f32),
}
impl CatPicker {
    pub fn new(selected: CatKind) -> Self {
        let thumbnails = CatKind::ALL
            .into_iter()
            .map(|kind| {
                let mut pm = Pixmap::new(108, 81).unwrap();
                if let Ok(art) = CatArt::load(kind) {
                    art.draw(
                        &mut pm,
                        0,
                        Transform::from_translate(0.0, -56.0).post_scale(0.45, 0.45),
                    );
                }
                pm
            })
            .collect();
        let mut picker = Self {
            selected,
            thumbnails,
            preview: Vec::new(),
            mode: 0,
            started: Instant::now(),
            previous_size: (0.0, 0.0),
        };
        picker.select(selected);
        picker
    }
    fn select(&mut self, kind: CatKind) {
        self.selected = kind;
        self.mode = 0;
        self.preview.clear();
        if let Ok(art) = CatArt::load(kind) {
            for frame in 0..16 {
                let mut pm = Pixmap::new(120, 90).unwrap();
                art.draw(
                    &mut pm,
                    frame,
                    Transform::from_translate(0.0, -56.0).post_scale(0.5, 0.5),
                );
                self.preview.push(pm);
            }
        }
    }
    pub fn navigate(&mut self, delta: isize) {
        let current = CatKind::ALL
            .iter()
            .position(|kind| *kind == self.selected)
            .unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(CatKind::ALL.len() as isize) as usize;
        self.select(CatKind::ALL[next]);
    }
    pub fn preview_mode(&mut self, mode: u8) {
        self.mode = mode.min(2);
        self.started = Instant::now();
    }
    pub fn frame(&self) -> usize {
        match self.mode {
            1 => {
                let progress = self.started.elapsed().as_secs_f32() / self.selected.copy_duration();
                if progress < 1.0 {
                    self.selected.frame(Some(progress), 0.0, 0.0, 0.0)
                } else {
                    0
                }
            }
            2 => 15,
            _ => 0,
        }
    }
    pub fn click(&mut self, x: f32, y: f32) -> Action {
        if (50.0..374.0).contains(&y) && (12.0..588.0).contains(&x) {
            let col = ((x - 12.0) / 144.0) as usize;
            let row = ((y - 50.0) / 108.0) as usize;
            if let Some(kind) = CatKind::ALL.get(row * 4 + col) {
                self.select(*kind);
            }
        }
        if (528.0..558.0).contains(&y) {
            if (12.0..92.0).contains(&x) {
                self.mode = 0;
            }
            if (98.0..178.0).contains(&x) {
                self.mode = 1;
                self.started = Instant::now();
            }
            if (184.0..264.0).contains(&x) {
                self.mode = 2;
            }
            if (318.0..496.0).contains(&x) {
                return Action::Apply(self.selected);
            }
            if (502.0..588.0).contains(&x) {
                return Action::Close;
            }
        }
        Action::None
    }
    pub fn draw(&self, pm: &mut Pixmap, origin: (f32, f32), lang: Lang) {
        self.draw_scaled(pm, origin, lang, 1.0);
    }
    /// Gallery text is rasterized at backing density; thumbnail caches stay bounded.
    pub fn draw_scaled(&self, pm: &mut Pixmap, origin: (f32, f32), lang: Lang, density: f32) {
        let ts = Transform::from_translate(origin.0, origin.1).post_scale(density, density);
        let rect = |pm: &mut Pixmap, x, y, w, h, color: (u8, u8, u8, u8)| {
            let mut paint = Paint::default();
            paint.set_color_rgba8(color.0, color.1, color.2, color.3);
            pm.fill_rect(Rect::from_xywh(x, y, w, h).unwrap(), &paint, ts, None);
        };
        let text = |pm: &mut Pixmap, s: &str, x, y, px| {
            sysfont::draw(pm, s, x, y, px, (67, 62, 57, 255), ts)
        };
        rect(pm, 0.0, 0.0, WIDTH, HEIGHT, (250, 248, 244, 255));
        text(pm, i18n::picker(lang, PickerText::Title), 16.0, 12.0, 2.0);
        text(pm, i18n::picker(lang, PickerText::Hint), 16.0, 33.0, 1.3);
        for (i, kind) in CatKind::ALL.into_iter().enumerate() {
            let x = 12.0 + (i % 4) as f32 * 144.0;
            let y = 50.0 + (i / 4) as f32 * 108.0;
            rect(
                pm,
                x,
                y,
                138.0,
                102.0,
                if kind == self.selected {
                    (210, 228, 241, 255)
                } else {
                    (238, 235, 229, 255)
                },
            );
            pm.draw_pixmap(
                0,
                0,
                self.thumbnails[i].as_ref(),
                &PixmapPaint::default(),
                Transform::from_translate(x + 15.0, y - 5.0).post_concat(ts),
                None,
            );
            let (a, b) = sysfont::wrap_two(kind.name(lang), 1.35, 130.0);
            text(pm, &a, x + 5.0, y + 76.0, 1.35);
            text(pm, &b, x + 5.0, y + 89.0, 1.35);
        }
        if let Some(frame) = self.preview.get(self.frame()) {
            pm.draw_pixmap(
                0,
                0,
                frame.as_ref(),
                &PixmapPaint::default(),
                Transform::from_translate(14.0, 405.0).post_concat(ts),
                None,
            );
        }
        text(pm, self.selected.name(lang), 148.0, 382.0, 1.8);
        text(
            pm,
            i18n::picker(lang, PickerText::Appearance),
            148.0,
            407.0,
            1.3,
        );
        let (a, b) = sysfont::wrap_two(self.selected.description(lang), 1.45, 430.0);
        text(pm, &a, 148.0, 424.0, 1.45);
        text(pm, &b, 148.0, 440.0, 1.45);
        text(
            pm,
            i18n::picker(lang, PickerText::Persona),
            148.0,
            464.0,
            1.3,
        );
        let (a, b) = sysfont::wrap_two(self.selected.persona(lang), 1.45, 430.0);
        text(pm, &a, 148.0, 481.0, 1.45);
        text(pm, &b, 148.0, 497.0, 1.45);
        for (x, w, label) in [
            (12.0, 80.0, PickerText::Idle),
            (98.0, 80.0, PickerText::Copy),
            (184.0, 80.0, PickerText::Sleep),
            (318.0, 178.0, PickerText::Apply),
            (502.0, 86.0, PickerText::Close),
        ] {
            rect(
                pm,
                x,
                528.0,
                w,
                30.0,
                if matches!(label, PickerText::Apply) {
                    (202, 225, 206, 255)
                } else {
                    (226, 223, 216, 255)
                },
            );
            text(pm, i18n::picker(lang, label), x + 9.0, 538.0, 1.45);
        }
    }
}
