//! Headless actual-cell-size idle/affection comparisons on light/dark backgrounds.
use clipcat::{cat::CatKind, hunter_art::CatArt};
use tiny_skia::{Color, Pixmap, Transform};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/clipcat-affection".into());
    std::fs::create_dir_all(&directory)?;
    let cats = [
        CatKind::Ragdoll,
        CatKind::Siamese,
        CatKind::Bombay,
        CatKind::BritishShorthair,
        CatKind::Birman,
        CatKind::Ragamuffin,
        CatKind::Persian,
        CatKind::TurkishAngora,
        CatKind::AmericanCurl,
        CatKind::Munchkin,
        CatKind::RussianBlue,
        CatKind::Tuxedo,
    ];
    for (group, kinds) in cats.chunks(4).enumerate() {
        for (name, color) in [
            ("light", Color::WHITE),
            ("dark", Color::from_rgba8(34, 39, 47, 255)),
        ] {
            let mut image = Pixmap::new(1200, 720).unwrap();
            image.fill(color);
            for (row, &kind) in kinds.iter().enumerate() {
                let art = CatArt::load(kind)?;
                for (column, frame) in [0, 16, 17, 18, 19].into_iter().enumerate() {
                    art.draw(
                        &mut image,
                        frame,
                        Transform::from_translate(column as f32 * 240., row as f32 * 180. - 56.),
                    );
                }
            }
            image.save_png(format!(
                "{directory}/preview-affection-{}-{name}.png",
                group + 1
            ))?;
        }
    }
    Ok(())
}
