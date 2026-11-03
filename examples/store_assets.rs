//! Render Store logos from the existing vector icon; no bundled art pipeline.
use tiny_skia::Pixmap;

fn main() {
    let dir = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&dir).unwrap();
    let source=Pixmap::load_png("art/originals/icon-brand-2026-11-04/selected-c.png").unwrap();
    for (name, size) in [("StoreLogo", 50), ("Square44x44Logo", 44), ("Square150x150Logo", 150)] {
        for scale in [1,2,4] {
            let pixels=size*scale;
            let mut image = Pixmap::new(pixels,pixels).unwrap();
            clipcat::brand_icon::draw_from(&mut image,clipcat::brand_icon::Surface::Windows,&source);
            let suffix=if scale==1 {String::new()} else {format!(".scale-{}",scale*100)};
            image.save_png(format!("{dir}/{name}{suffix}.png")).unwrap();
        }
    }
    for size in [16,20,24,32,40,48,64,96,256] {
        let mut image=Pixmap::new(size,size).unwrap();
        clipcat::brand_icon::draw_from(&mut image,clipcat::brand_icon::Surface::Windows,&source);
        for suffix in ["", "_altform-unplated"] {
            image.save_png(format!("{dir}/Square44x44Logo.targetsize-{size}{suffix}.png")).unwrap();
        }
    }
}
