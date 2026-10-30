//! A replaceable cache of the selected character's normalized atlas frames.
//! Production owners retain only the active character, and drop it on replacement.
use crate::cat::CatKind;
use tiny_skia::{FilterQuality, IntRect, Pixmap, PixmapPaint, Transform};
pub const FRAME_COUNT: usize = 20;
pub const FRAME_W: u32 = 240;
pub const FRAME_H: u32 = 180;
pub struct CatArt {
    frames: Vec<Pixmap>,
}
impl CatArt {
    pub fn load(kind: CatKind) -> Result<Self, String> {
        Self::load_subset(kind, usize::MAX)
    }
    /// Sequential picker decode retains only the requested compact source frames.
    pub fn load_thumbnail(kind: CatKind) -> Result<Self, String> {
        Self::load_subset(kind, 1)
    }
    pub fn load_preview(kind: CatKind) -> Result<Self, String> {
        Self::load_subset(kind, 16)
    }
    fn load_subset(kind: CatKind, limit: usize) -> Result<Self, String> {
        let bytes: &[u8] = match kind {
            CatKind::Ragdoll => include_bytes!("../assets/cats/ragdoll.png"),
            CatKind::Birman => include_bytes!("../assets/cats/birman.png"),
            CatKind::Siamese => include_bytes!("../assets/cats/siamese.png"),
            CatKind::Bombay => include_bytes!("../assets/cats/bombay.png"),
            CatKind::Ragamuffin => include_bytes!("../assets/cats/ragamuffin.png"),
            CatKind::Persian => include_bytes!("../assets/cats/persian.png"),
            CatKind::TurkishAngora => include_bytes!("../assets/cats/turkish-angora.png"),
            CatKind::AmericanCurl => include_bytes!("../assets/cats/american-curl.png"),
            CatKind::Munchkin => include_bytes!("../assets/cats/munchkin.png"),
            CatKind::BritishShorthair => include_bytes!("../assets/cats/british-shorthair.png"),
            CatKind::RussianBlue => include_bytes!("../assets/cats/russian-blue.png"),
            CatKind::Tuxedo => include_bytes!("../assets/cats/tuxedo.png"),
        };
        let atlas = Pixmap::decode_png(bytes).map_err(|error| error.to_string())?;
        let count = (atlas.height() / FRAME_H * 4) as usize;
        if atlas.width() != 960 || !atlas.height().is_multiple_of(FRAME_H)
            || !(20..=24).contains(&count) {
            return Err("cat atlas must have 20–24 complete 240×180 frames".into());
        }
        let frames = (0..count.min(limit))
            .map(|i| {
                atlas
                    .clone_rect(
                        IntRect::from_xywh((i as i32 % 4) * 240, (i as i32 / 4) * 180, 240, 180)
                            .unwrap(),
                    )
                    .ok_or_else(|| "missing atlas frame".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { frames })
    }
    pub fn hit(&self, frame: usize, x: f32, y: f32) -> bool {
        let y = y - 56.;
        x >= 0. && y >= 0. && x < FRAME_W as f32 && y < FRAME_H as f32
            && self.frames[frame.min(self.frames.len()-1)].pixel(x as u32,y as u32)
                .is_some_and(|pixel| pixel.alpha() > 32)
    }
    pub fn cache_bytes(&self) -> usize {
        self.frames.iter().map(|p| p.data().len()).sum()
    }
    pub fn draw(&self, target: &mut Pixmap, frame: usize, transform: Transform) {
        target.draw_pixmap(
            0,
            56,
            self.frames[frame.min(self.frames.len() - 1)].as_ref(),
            &PixmapPaint {
                quality: FilterQuality::Bilinear,
                ..Default::default()
            },
            transform,
            None,
        );
    }
}
#[cfg(test)]
mod selected_tests {
    use super::*;
    #[test]
    fn selected_art_loads_every_kind_with_bounded_cache_and_clear_margins() {
        for kind in crate::cat::CatKind::ALL {
            let art = CatArt::load(kind).unwrap();
            assert!(art.cache_bytes() < 4 * 1024 * 1024);
            assert_eq!(art.frames.len(), 20);
            for (i, frame) in art.frames.iter().enumerate() {
                assert_eq!((frame.width(), frame.height()), (240, 180));
                let occupied: Vec<_> = (0..180)
                    .filter(|&y| (0..240).any(|x| frame.pixel(x, y).unwrap().alpha() > 128))
                    .collect();
                assert!(occupied[0] > 0, "{} frame {i} top", kind.id());
                assert!(
                    (174..=176).contains(occupied.last().unwrap()),
                    "{} frame {i} baseline {:?}",
                    kind.id(),
                    occupied.last()
                );
                assert!((0..180).all(|y| frame.pixel(0, y).unwrap().alpha() == 0
                    && frame.pixel(239, y).unwrap().alpha() == 0));
            }
        }
    }
}
