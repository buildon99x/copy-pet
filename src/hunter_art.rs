//! The selected illustrated ragdoll. Decode the embedded atlas once and retain
//! only small premultiplied frames, not the full source image (ADR-0017).
use std::sync::OnceLock;
use tiny_skia::{FilterQuality, IntRect, Pixmap, PixmapPaint, Transform};

pub const FRAME_COUNT: usize = 16;
pub const FRAME_W: u32 = 240;
pub const FRAME_H: u32 = 180;

/// Hand-checked nose centers on the normalized art, in the 240×256 pet
/// canvas. Accessories follow the head rather than the frame bounding box.
pub fn face_anchor(frame: usize) -> (f32, f32) {
    const NOSES: [(f32, f32); FRAME_COUNT] = [
        (155.0, 158.0),
        (164.0, 159.0),
        (162.0, 163.0),
        (159.0, 189.0),
        (168.0, 196.0),
        (169.0, 194.0),
        (176.0, 185.0),
        (170.0, 190.0),
        (163.0, 162.0),
        (160.0, 161.0),
        (158.0, 164.0),
        (159.0, 161.0),
        (155.0, 168.0),
        (151.0, 171.0),
        (148.0, 166.0),
        (159.0, 205.0),
    ];
    NOSES[frame.min(FRAME_COUNT - 1)]
}

fn frames() -> &'static [Pixmap] {
    static FRAMES: OnceLock<Vec<Pixmap>> = OnceLock::new();
    FRAMES.get_or_init(|| {
        let atlas = Pixmap::decode_png(include_bytes!("../assets/cottonball-hunter.png"))
            .expect("the bundled ragdoll atlas is validated by tests");
        // The hand-drawn sheet has generous gaps, but its first row extends
        // below the nominal 256px cell. These measured separators stay in the
        // transparent gutters and never cut paws, ears or tails.
        let xs = [0, 390, 780, 1168, 1536];
        let ys = [0, 280, 512, 776, 1024];
        let mut tiles = Vec::with_capacity(FRAME_COUNT);
        for row in 0..4 {
            for col in 0..4 {
                let tile = atlas
                    .clone_rect(
                        IntRect::from_ltrb(xs[col], ys[row], xs[col + 1], ys[row + 1]).unwrap(),
                    )
                    .unwrap();
                let mut left = tile.width();
                let mut top = tile.height();
                let (mut right, mut bottom) = (0, 0);
                for y in 0..tile.height() {
                    for x in 0..tile.width() {
                        if tile.pixel(x, y).unwrap().alpha() > 128 {
                            left = left.min(x);
                            top = top.min(y);
                            right = right.max(x + 1);
                            bottom = bottom.max(y + 1);
                        }
                    }
                }
                // Ignore low-alpha wisps in the gutter when locating feet;
                // retain two pixels around the opaque bounds for edge AA.
                let rect = IntRect::from_ltrb(
                    left.saturating_sub(2) as i32,
                    top.saturating_sub(2) as i32,
                    (right + 2).min(tile.width()) as i32,
                    (bottom + 2).min(tile.height()) as i32,
                )
                .expect("each atlas cell contains an opaque character");
                tiles.push(tile.clone_rect(rect).unwrap());
            }
        }
        // One scale for the entire atlas: never enlarge a crouch to match a
        // sitting pose. Align feet on a common line to avoid vertical jitter.
        let max_w = tiles.iter().map(Pixmap::width).max().unwrap() as f32;
        let max_h = tiles.iter().map(Pixmap::height).max().unwrap() as f32;
        let scale = (224.0 / max_w).min(174.0 / max_h);
        tiles
            .into_iter()
            .map(|tile| {
                let mut out = Pixmap::new(FRAME_W, FRAME_H).unwrap();
                let x = (FRAME_W as f32 - tile.width() as f32 * scale) / 2.0;
                let y = 176.0 - tile.height() as f32 * scale;
                out.draw_pixmap(
                    0,
                    0,
                    tile.as_ref(),
                    &PixmapPaint {
                        quality: FilterQuality::Bicubic,
                        ..PixmapPaint::default()
                    },
                    Transform::from_scale(scale, scale).post_translate(x, y),
                    None,
                );
                out
            })
            .collect()
    })
}

pub fn draw(target: &mut Pixmap, frame: usize, transform: Transform) {
    target.draw_pixmap(
        0,
        56,
        frames()[frame.min(FRAME_COUNT - 1)].as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        },
        transform,
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn illustrated_cache_is_bounded_transparent_and_keeps_all_feet_on_one_line() {
        let frames = frames();
        assert_eq!(frames.len(), 16);
        assert!(frames.iter().map(|p| p.data().len()).sum::<usize>() < 4 * 1024 * 1024);
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(
                frame.pixel(0, 0).unwrap().alpha(),
                0,
                "frame {i} has clear margins"
            );
            let occupied: Vec<_> = (0..FRAME_H)
                .filter(|&y| (0..FRAME_W).any(|x| frame.pixel(x, y).unwrap().alpha() > 128))
                .collect();
            assert!(
                (174..=176).contains(occupied.last().unwrap()),
                "frame {i} baseline"
            );
            assert!(occupied[0] > 0, "frame {i} has room above its ears");
            assert!((0..FRAME_H).all(|y| frame.pixel(0, y).unwrap().alpha() == 0
                && frame.pixel(FRAME_W - 1, y).unwrap().alpha() == 0));
        }
    }
}
