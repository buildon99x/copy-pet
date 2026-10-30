//! Offline atlas preparation. SOURCE DEST [SEPARATORS.json].
//! Optional JSON accepts {"cells":[[x,y,w,h],...]} with exactly sixteen
//! row-major rectangles, or global separators.
//! Global separator example: {"xs":[0,390,780,1168,1536],"ys":[0,280,512,776,1024]}.
use tiny_skia::{FilterQuality, IntRect, Pixmap, PixmapPaint, Transform};
/// Keep the character and its connected AA/whiskers, discarding neighboring
/// sprite fragments. Connectivity uses alpha > 0 and eight neighbors: an
/// opaque-only mask would split fine whiskers and antialiased diagonal edges.
pub(crate) fn retain_largest_component(tile: &mut Pixmap) {
    let (width, height) = (tile.width() as usize, tile.height() as usize);
    let mut seen = vec![false; width * height];
    let mut largest = Vec::new();
    for seed in 0..seen.len() {
        if seen[seed] || tile.pixels()[seed].alpha() == 0 {
            continue;
        }
        seen[seed] = true;
        let mut component = vec![seed];
        let mut cursor = 0;
        while cursor < component.len() {
            let at = component[cursor];
            cursor += 1;
            let (x, y) = (at % width, at / width);
            for yy in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let next = yy * width + xx;
                    if !seen[next] && tile.pixels()[next].alpha() > 0 {
                        seen[next] = true;
                        component.push(next);
                    }
                }
            }
        }
        if component.len() > largest.len() {
            largest = component;
        }
    }
    let mut keep = vec![false; seen.len()];
    for at in largest {
        keep[at] = true;
    }
    for (at, pixel) in tile.data_mut().chunks_exact_mut(4).enumerate() {
        if !keep[at] {
            pixel.fill(0);
        }
    }
}
fn prepare(source: &Pixmap, xs: [u32; 5], ys: [u32; 5]) -> Result<Pixmap, String> {
    if xs.windows(2).any(|v| v[0] >= v[1]) || ys.windows(2).any(|v| v[0] >= v[1]) {
        return Err("invalid separators".into());
    }
    let cells = std::array::from_fn(|i| {
        let (col, row) = (i % 4, i / 4);
        [
            xs[col],
            ys[row],
            xs[col + 1] - xs[col],
            ys[row + 1] - ys[row],
        ]
    });
    prepare_rects(source, cells)
}
fn prepare_rects(source: &Pixmap, cells: [[u32; 4]; 16]) -> Result<Pixmap, String> {
    let mut tiles = Vec::with_capacity(16);
    for (index, [x, y, w, h]) in cells.into_iter().enumerate() {
        if w == 0
            || h == 0
            || x.checked_add(w).is_none_or(|r| r > source.width())
            || y.checked_add(h).is_none_or(|b| b > source.height())
        {
            return Err(format!("cell {index} outside source or empty"));
        }
        let rect = IntRect::from_xywh(x as i32, y as i32, w, h).ok_or("invalid cell rectangle")?;
        let mut tile = source.clone_rect(rect).ok_or("cell outside source")?;
        retain_largest_component(&mut tile);
        let (mut left, mut top, mut right, mut bottom) = (tile.width(), tile.height(), 0, 0);
        for y in 0..tile.height() {
            for x in 0..tile.width() {
                // Disconnected noise is gone; keep all connected AA and whiskers.
                if tile.pixel(x, y).unwrap().alpha() > 0 {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x + 1);
                    bottom = bottom.max(y + 1);
                }
            }
        }
        if right == 0 {
            return Err(format!("empty cell {index}"));
        }
        let bounds = IntRect::from_ltrb(
            left.saturating_sub(2) as i32,
            top.saturating_sub(2) as i32,
            (right + 2).min(tile.width()) as i32,
            (bottom + 2).min(tile.height()) as i32,
        )
        .unwrap();
        tiles.push(tile.clone_rect(bounds).unwrap());
    }
    let max_w = tiles.iter().map(Pixmap::width).max().unwrap() as f32;
    let max_h = tiles.iter().map(Pixmap::height).max().unwrap() as f32;
    let scale = (224. / max_w).min(174. / max_h);
    let mut atlas = Pixmap::new(960, 720).unwrap();
    for (i, tile) in tiles.iter().enumerate() {
        let x = (240. - tile.width() as f32 * scale) / 2.;
        let y = 176. - tile.height() as f32 * scale;
        let mut normalized = Pixmap::new(240, 180).unwrap();
        normalized.draw_pixmap(
            0,
            0,
            tile.as_ref(),
            &PixmapPaint {
                quality: FilterQuality::Bicubic,
                ..Default::default()
            },
            Transform::from_scale(scale, scale).post_translate(x, y),
            None,
        );
        // Padding and alpha interpolation change the visible foot position.
        // Measure after resampling, then translate by whole pixels so that
        // filtering cannot introduce a second baseline error.
        let occupied: Vec<_> = (0..180)
            .filter(|&row| (0..240).any(|col| normalized.pixel(col, row).unwrap().alpha() > 128))
            .collect();
        let bottom = *occupied
            .last()
            .ok_or_else(|| format!("cell {i} vanished after resampling"))?;
        let shift = 175 - bottom as i32;
        let first_alpha = (0..180)
            .find(|&row| (0..240).any(|col| normalized.pixel(col, row).unwrap().alpha() > 0))
            .unwrap();
        if first_alpha as i32 + shift < 0 {
            return Err(format!(
                "cell {i} has insufficient top margin for foot alignment"
            ));
        }
        let mut aligned = Pixmap::new(240, 180).unwrap();
        aligned.draw_pixmap(
            0,
            shift,
            normalized.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        atlas.draw_pixmap(
            (i % 4) as i32 * 240,
            (i / 4) as i32 * 180,
            aligned.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    Ok(atlas)
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 || args.len() > 4 {
        return Err("usage: prepare_cat_atlas SOURCE DEST [SEPARATORS.json]".into());
    }
    let source = Pixmap::load_png(&args[1]).map_err(|e| e.to_string())?;
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Cuts {
        Cells { cells: [[u32; 4]; 16] },
        Grid { xs: [u32; 5], ys: [u32; 5] },
    }
    let cuts = if let Some(path) = args.get(3) {
        serde_json::from_slice::<Cuts>(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        Cuts::Grid {
            xs: std::array::from_fn(|i| source.width() * i as u32 / 4),
            ys: std::array::from_fn(|i| source.height() * i as u32 / 4),
        }
    };
    let atlas = match cuts {
        Cuts::Cells { cells } => prepare_rects(&source, cells),
        Cuts::Grid { xs, ys } => prepare(&source, xs, ys),
    }?;
    atlas.save_png(&args[2]).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disconnected_fragments_are_removed_without_losing_connected_low_alpha_whiskers() {
        let mut source = Pixmap::new(400, 400).unwrap();
        for i in 0..16 {
            let (x, y) = ((i % 4) * 100, (i / 4) * 100);
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(255, 0, 0, 255);
            source.fill_rect(
                tiny_skia::Rect::from_xywh((x + 20) as f32, (y + 20) as f32, 40., 50.).unwrap(),
                &paint,
                Transform::identity(),
                None,
            );
            paint.set_color_rgba8(0, 0, 255, 60);
            source.fill_rect(
                tiny_skia::Rect::from_xywh((x + 59) as f32, (y + 30) as f32, 16., 1.).unwrap(),
                &paint,
                Transform::identity(),
                None,
            );
            paint.set_color_rgba8(0, 0, 0, 255);
            source.fill_rect(
                tiny_skia::Rect::from_xywh((x + 5) as f32, (y + 5) as f32, 8., 2.).unwrap(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        let mut first = source
            .clone_rect(IntRect::from_xywh(0, 0, 100, 100).unwrap())
            .unwrap();
        let whisker = first.pixel(70, 30).unwrap();
        retain_largest_component(&mut first);
        assert_eq!(
            first.pixel(7, 5).unwrap().alpha(),
            0,
            "disconnected neighboring black dash is removed"
        );
        assert_eq!(
            first.pixel(70, 30).unwrap(),
            whisker,
            "connected low-alpha whiskers retain their exact pixels"
        );
        let atlas = prepare(&source, [0, 100, 200, 300, 400], [0, 100, 200, 300, 400]).unwrap();
        assert!(
            !atlas
                .pixels()
                .iter()
                .any(|p| p.alpha() > 128 && p.red() < 5 && p.green() < 5 && p.blue() < 5),
            "no opaque black fragment survives normalization"
        );
        assert!(
            atlas
                .pixels()
                .iter()
                .any(|p| p.alpha() > 128 && p.red() > 200),
            "the main character remains"
        );
        let blue_span = (0..180)
            .map(|y| {
                (0..240)
                    .filter(|&x| {
                        let p = atlas.pixel(x, y).unwrap();
                        p.alpha() > 8 && p.blue() > p.red()
                    })
                    .count()
            })
            .max()
            .unwrap();
        assert!(blue_span>25,"the final normalized frame must retain the long low-alpha whisker; found {blue_span}px");
    }
    #[test]
    fn opaque_feet_align_after_resampling_soft_alpha_edges() {
        let mut source = Pixmap::new(400, 400).unwrap();
        for i in 0..16 {
            let x = (i % 4 * 100 + 30) as f32;
            let y = (i / 4 * 100 + 20) as f32;
            let height = if i == 0 { 60. } else { 40. };
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(90, 90, 90, 255);
            source.fill_rect(
                tiny_skia::Rect::from_xywh(x, y, 30., height).unwrap(),
                &paint,
                Transform::identity(),
                None,
            );
            // A barely opaque foot edge moves under bicubic resampling, so
            // aligning the padded source bounds cannot align visible feet.
            paint.set_color_rgba8(90, 90, 90, 140);
            source.fill_rect(
                tiny_skia::Rect::from_xywh(x, y + height, 30., 1.).unwrap(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        let atlas = prepare(&source, [0, 100, 200, 300, 400], [0, 100, 200, 300, 400]).unwrap();
        for i in 0..16 {
            let occupied: Vec<_> = (0..180)
                .filter(|&y| {
                    (0..240).any(|x| {
                        atlas
                            .pixel((i % 4) * 240 + x, (i / 4) * 180 + y)
                            .unwrap()
                            .alpha()
                            > 128
                    })
                })
                .collect();
            assert_eq!(
                occupied.last(),
                Some(&175),
                "frame {i} opaque foot baseline after resampling"
            );
            assert!(occupied[0] > 0, "frame {i} retains top margin");
            assert!((0..240).all(|x| atlas
                .pixel((i % 4) * 240 + x, (i / 4) * 180)
                .unwrap()
                .alpha()
                == 0
                && atlas
                    .pixel((i % 4) * 240 + x, (i / 4) * 180 + 179)
                    .unwrap()
                    .alpha()
                    == 0));
        }
    }
    #[test]
    fn asymmetric_row_cuts_keep_a_colored_paw_separate_from_the_next_tail() {
        let mut source = Pixmap::new(400, 400).unwrap();
        let mut cells =
            std::array::from_fn(|i| [(i as u32 % 4) * 100, (i as u32 / 4) * 100, 100, 100]);
        cells[0] = [0, 0, 110, 100];
        cells[1] = [110, 0, 90, 100];
        cells[4] = [0, 100, 90, 100];
        cells[5] = [90, 100, 110, 100];
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(80, 80, 80, 255);
        for i in 0..16 {
            source.fill_rect(
                tiny_skia::Rect::from_xywh(
                    (i % 4 * 100 + 30) as f32,
                    (i / 4 * 100 + 30) as f32,
                    30.,
                    40.,
                )
                .unwrap(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        // Attach the test paw and tail to their own bodies, as in source art.
        for (x, w) in [(50., 50.), (120., 11.)] {
            source.fill_rect(
                tiny_skia::Rect::from_xywh(x, 65., w, 5.).unwrap(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        // A paw crosses the regular x=100 cut only in the first row.
        paint.set_color_rgba8(255, 0, 0, 255);
        source.fill_rect(
            tiny_skia::Rect::from_xywh(98., 65., 10., 10.).unwrap(),
            &paint,
            Transform::identity(),
            None,
        );
        // The neighboring tail is close, but belongs wholly to frame one.
        paint.set_color_rgba8(0, 0, 255, 255);
        source.fill_rect(
            tiny_skia::Rect::from_xywh(112., 65., 10., 10.).unwrap(),
            &paint,
            Transform::identity(),
            None,
        );
        let atlas = prepare_rects(&source, cells).unwrap();
        let count_color = |frame: u32, red: bool| {
            (0..180)
                .flat_map(|y| (0..240).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let p = atlas.pixel(frame * 240 + x, y).unwrap();
                    p.alpha() > 200
                        && if red {
                            p.red() > 200 && p.blue() < 20
                        } else {
                            p.blue() > 200 && p.red() < 20
                        }
                })
                .count()
        };
        assert!(
            count_color(0, true) > 150,
            "the complete first paw must survive its wider row cut"
        );
        assert_eq!(
            count_color(0, false),
            0,
            "the next tail must not leak into frame zero"
        );
        assert_eq!(
            count_color(1, true),
            0,
            "the paw must not leak into its neighbor"
        );
        assert!(
            count_color(1, false) > 150,
            "the neighboring tail must survive"
        );
    }
    #[test]
    fn empty_sheet_is_rejected() {
        assert!(prepare(
            &Pixmap::new(16, 16).unwrap(),
            [0, 4, 8, 12, 16],
            [0, 4, 8, 12, 16]
        )
        .is_err());
    }
    #[test]
    fn one_scale_preserves_pose_size_and_baseline() {
        let mut source = Pixmap::new(400, 400).unwrap();
        for i in 0..16 {
            let w = if i == 0 { 40 } else { 20 };
            let r = tiny_skia::Rect::from_xywh(
                (i % 4 * 100 + 25) as f32,
                (i / 4 * 100 + 30) as f32,
                w as f32,
                40.,
            )
            .unwrap();
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(1, 2, 3, 255);
            source.fill_rect(r, &paint, Transform::identity(), None);
        }
        let out = prepare(&source, [0, 100, 200, 300, 400], [0, 100, 200, 300, 400]).unwrap();
        let width = |col: u32| {
            (0..240)
                .filter(|x| (0..180).any(|y| out.pixel(col * 240 + x, y).unwrap().alpha() > 128))
                .count()
        };
        assert!(width(0) > width(1) * 3 / 2);
        for i in 0..16 {
            let ys: Vec<_> = (0..180)
                .filter(|y| {
                    (0..240).any(|x| {
                        out.pixel((i % 4) * 240 + x, (i / 4) * 180 + y)
                            .unwrap()
                            .alpha()
                            > 128
                    })
                })
                .collect();
            assert!((166..=176).contains(ys.last().unwrap()));
        }
    }
}
