//! Synthetic read-only UI review: native 1x/2x raster output from the real Pet APIs.
//! cargo run --locked --release --example review_at_density -- /tmp/clipcat-density
use clipcat::{clipboard::ClipStore, i18n::Lang, pet::Pet, state::Persist, viewport::Density};
use tiny_skia::Pixmap;

fn save(pet: &Pet, dir: &str, name: &str, flyout: bool) {
    for scale in [1.0, 2.0] {
        let size = if flyout {
            pet.flyout_size()
        } else {
            pet.canvas_size()
        };
        let (w, h) = Density::new(scale).physical_size(size);
        let mut pm = Pixmap::new(w, h).unwrap();
        if flyout {
            pet.render_flyout_at_density(&mut pm, scale as f32);
        } else {
            pet.render_at_density(&mut pm, scale as f32);
        }
        let path = format!("{dir}/{name}-{}x.png", scale as u32);
        pm.save_png(&path).unwrap();
        println!(
            "{path}: {w}x{h} pixels / {}x{} logical units",
            size.0, size.1
        );
    }
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/clipcat-density".into());
    std::fs::create_dir_all(&dir).unwrap();
    for (lang, label) in [(Lang::En, "en"), (Lang::Ko, "ko")] {
        let mut st = Persist::default();
        st.set_lang(lang);
        st.onboarded = true;
        let mut p = Pet::new(st);
        p.clips = ClipStore::default();
        p.workbench = Default::default();
        for text in [
            "Readable synthetic clipboard text",
            "한글 글꼴과 영어를 함께 확인합니다",
            "https://example.com/review",
            "Native density keeps logical hit regions unchanged",
        ] {
            p.clips.add_copy(text.into(), Some("Review".into()));
        }
        p.open_flyout();
        save(&p, &dir, &format!("flyout-{label}"), true);
        p.open_workbench();
        save(&p, &dir, &format!("workbench-{label}"), true);
        p.close_flyout();
        p.workbench = Default::default();
        p.panel.ai_view = None;
        p.open_panel();
        save(&p, &dir, &format!("embedded-{label}"), false);
        p.open_cat_picker();
        save(&p, &dir, &format!("gallery-{label}"), false);
    }
}
