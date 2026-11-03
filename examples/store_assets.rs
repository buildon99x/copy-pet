//! Render Store logos from the existing vector icon; no bundled art pipeline.
use tiny_skia::Pixmap;

fn main() {
    let dir = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, size) in [("StoreLogo", 50), ("Square44x44Logo", 44), ("Square150x150Logo", 150)] {
        let mut image = Pixmap::new(size, size).unwrap();
        clipcat::render::draw_icon_scaled(&mut image, size as f32 / 32.0);
        image.save_png(format!("{dir}/{name}.png")).unwrap();
    }
}
