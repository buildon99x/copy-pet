//! System-font inventory and direct rasterization samples; no window required.
//! cargo run --locked --release --example font_diagnostics -- [output-directory]
use clipcat::sysfont;
use tiny_skia::{Pixmap, Transform};

fn main() {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/clipcat-font-diagnostics".into());
    std::fs::create_dir_all(&directory).unwrap();
    let report = sysfont::diagnostics();
    print!("{report}");
    std::fs::write(format!("{directory}/fonts.txt"), report).unwrap();
    for scale in [1u32, 2] {
        let mut pm = Pixmap::new(680 * scale, 190 * scale).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(248, 250, 252, 255));
        for (row, text) in [
            "Clipboard / English / Regular / 0123456789",
            "클립보드 / 안녕하세요 / 한글 / 0123456789",
            "Mixed clipboard 한글 English 123, punctuation !?",
            "Body 1.4: The quick brown fox / 클립캣 복사한 내용",
            "13pt: Clipboard 클립보드 AaBb 0123456789",
        ]
        .into_iter()
        .enumerate()
        {
            let size = if row == 4 { 13.0 / 9.1 } else { 1.4 };
            sysfont::draw(
                &mut pm,
                text,
                14.0,
                12.0 + row as f32 * 34.0,
                size,
                (30, 35, 40, 255),
                Transform::from_scale(scale as f32, scale as f32),
            );
        }
        let path = format!("{directory}/fonts-{scale}x.png");
        pm.save_png(&path).unwrap();
        println!("wrote {path}");
    }
}
