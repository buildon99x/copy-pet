//! Offline packaging: prepare_affection SOURCE_DIRECTORY [ASSET_DIRECTORY].
//! Measured cuts and breed row order are retained in affection-source.json.
#[allow(dead_code)]
#[path = "prepare_cat_atlas.rs"]
mod prepare;
use tiny_skia::{FilterQuality, IntRect, Pixmap, PixmapPaint, Transform};
fn bounds(pm: &Pixmap) -> (u32, u32, u32, u32) {
    let (mut l, mut t, mut r, mut b) = (pm.width(), pm.height(), 0, 0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            if pm.pixel(x, y).unwrap().alpha() > 128 {
                l = l.min(x);
                t = t.min(y);
                r = r.max(x + 1);
                b = b.max(y + 1);
            }
        }
    }
    (l, t, r, b)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let source = args.get(1).ok_or("source directory required")?;
    let dest = args.get(2).map(String::as_str).unwrap_or("assets/cats");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(format!("{source}/manifest.json"))?)?;
    let mut metadata = Vec::new();
    let mut review = Pixmap::new(1200, 12 * 180).unwrap();
    review.fill(tiny_skia::Color::WHITE);
    let mut review_row = 0;
    for sheet in manifest["sources"].as_array().unwrap() {
        let file = sheet["file"].as_str().unwrap();
        let pm = Pixmap::load_png(format!("{source}/{file}"))?;
        let cuts: serde_json::Value = serde_json::from_slice(&std::fs::read(format!(
            "{source}/{}",
            file.replace(".png", ".cuts.json")
        ))?)?;
        for (row, kind) in sheet["rows"].as_array().unwrap().iter().enumerate() {
            let kind = kind.as_str().unwrap();
            let path = format!("{dest}/{kind}.png");
            let old = Pixmap::load_png(&path)?;
            let idle = old
                .clone_rect(IntRect::from_xywh(0, 0, 240, 180).unwrap())
                .unwrap();
            let (_, idle_top, _, idle_bottom) = bounds(&idle);
            let mut tiles = Vec::new();
            let mut rects = Vec::new();
            for col in 0..4 {
                let c = &cuts["cells"][row * 4 + col];
                let r: Vec<_> = c
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_u64().unwrap() as u32)
                    .collect();
                let mut tile = pm
                    .clone_rect(IntRect::from_xywh(r[0] as i32, r[1] as i32, r[2], r[3]).unwrap())
                    .unwrap();
                prepare::retain_largest_component(&mut tile);
                let (l, t, r, b) = bounds(&tile);
                tiles.push(
                    tile.clone_rect(
                        IntRect::from_ltrb(
                            l.saturating_sub(2) as i32,
                            t.saturating_sub(2) as i32,
                            (r + 2).min(tile.width()) as i32,
                            (b + 2).min(tile.height()) as i32,
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                );
                rects.push(c.clone());
            }
            // One scale for all four poses; match eye-contact body height to idle.
            let scale = ((idle_bottom - idle_top) as f32 / tiles[0].height() as f32)
                .min(224. / tiles.iter().map(Pixmap::width).max().unwrap() as f32)
                .min(172. / tiles.iter().map(Pixmap::height).max().unwrap() as f32);
            let mut atlas = Pixmap::new(960, 900).unwrap();
            let original = old
                .clone_rect(IntRect::from_xywh(0, 0, 960, 720).unwrap())
                .unwrap();
            atlas.draw_pixmap(
                0,
                0,
                original.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            review.draw_pixmap(
                0,
                review_row * 180,
                idle.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            for (col, tile) in tiles.iter().enumerate() {
                let mut normalized = Pixmap::new(240, 180).unwrap();
                normalized.draw_pixmap(
                    0,
                    0,
                    tile.as_ref(),
                    &PixmapPaint {
                        quality: FilterQuality::Bicubic,
                        ..Default::default()
                    },
                    Transform::from_scale(scale, scale).post_translate(
                        (240. - tile.width() as f32 * scale) / 2.,
                        175. - tile.height() as f32 * scale,
                    ),
                    None,
                );
                let bottom = (0..180)
                    .rev()
                    .find(|&y| (0..240).any(|x| normalized.pixel(x, y).unwrap().alpha() > 128))
                    .unwrap();
                let mut aligned = Pixmap::new(240, 180).unwrap();
                aligned.draw_pixmap(
                    0,
                    175 - bottom as i32,
                    normalized.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
                atlas.draw_pixmap(
                    col as i32 * 240,
                    720,
                    aligned.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
                review.draw_pixmap(
                    (col as i32 + 1) * 240,
                    review_row * 180,
                    aligned.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
            atlas.save_png(path)?;
            metadata.push(serde_json::json!({"cat":kind,"source":file,"cells":rects,"scale":scale,"baseline":175,"roles":["eye-contact","gesture","contentment","settle"],"original_frames":16,"affection_frames":4}));
            review_row += 1;
        }
    }
    std::fs::write(
        format!("{dest}/affection-source.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    review.save_png("/tmp/clipcat-affection-review.png")?;
    Ok(())
}
