//! Dev tool: renders representative frames (cat, fish, panel, stats bubble,
//! Hangul text) to PNGs so changes can be eyeballed without launching the
//! app — useful in headless environments.
//!
//! Usage: cargo run --release --example preview [out_dir]

use clipcat::clipboard::ClipStore;
use clipcat::i18n::Lang;
use clipcat::panel::Panel;
use clipcat::render::{self, Accessory, Badge, BubbleData, FishView, PanelView, Scene};
use tiny_skia::Pixmap;

fn base_scene(lang: Lang) -> Scene<'static> {
    Scene {
        paw_l: 0.0,
        paw_r: 0.6,
        blink: 0.0,
        happy: 0.0,
        sleep: 0.0,
        excite: 0.0,
        squash: 0.0,
        breath: 0.3,
        tail_phase: 1.2,
        mouth_open: 0.0,
        accessory: Accessory::Scarf,
        particles: &[],
        fish: None,
        bubble: None,
        bubble_alpha: 0.0,
        toast: None,
        hotkey_hint: None,
        lang,
        origin: (0.0, 0.0),
    }
}

fn save(pm: &Pixmap, dir: &str, name: &str) {
    let path = format!("{dir}/{name}.png");
    pm.save_png(&path).unwrap();
    println!("wrote {path}");
}

fn demo_store() -> ClipStore {
    let mut store = ClipStore::default();
    store.add_copy("fn main() {\n    println!(\"hello\");\n}".into(), Some("Code".into()));
    store.add_copy("안녕하세요! 클립캣입니다. 이 줄은 길어서 잘리는지 확인합니다 — 아주 아주 길게".into(), Some("브라우저".into()));
    store.add_copy("https://example.com/some/long/path?q=clipboard&lang=ko".into(), Some("chrome".into()));
    store.add_copy("두 번째 클립".into(), None);
    store.add_copy("short".into(), Some("terminal".into()));
    store.add_copy("일곱 번째 줄까지 스크롤 테스트".into(), Some("notes".into()));
    store.add_copy("and one more to overflow the list".into(), Some("mail".into()));
    store.add_copy("a big clip ".repeat(100), Some("Code".into())); // shows the size meta
    store.add_copy("아홉 번째 클립".into(), Some("터미널".into()));
    store.add_copy("tenth clip scrolls the list".into(), Some("Chrome".into()));
    store.add_copy("eleventh clip - no quick badge".into(), Some("Code".into()));
    let pins: Vec<u64> = store.visible("").iter().rev().take(1).map(|c| c.id).collect();
    for id in pins {
        store.toggle_pin(id);
    }
    store
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/clipcat-preview".into());
    std::fs::create_dir_all(&dir).unwrap();

    // 1. text rendering: the system font (every UI surface) at several sizes
    {
        let mut pm = Pixmap::new(560, 200).unwrap();
        pm.fill(tiny_skia::Color::WHITE);
        let ts = tiny_skia::Transform::identity();
        let black = (40, 40, 40, 255);
        println!("system font available: {}", clipcat::sysfont::available());
        clipcat::sysfont::draw(&mut pm, "클립보드 히스토리 검색...", 10.0, 10.0, 2.0, black, ts);
        clipcat::sysfont::draw(&mut pm, "복사하면 여기에 쌓여요! Copy lands here 123", 10.0, 38.0, 1.6, black, ts);
        clipcat::sysfont::draw(&mut pm, "The quick brown fox jumps over the lazy dog", 10.0, 62.0, 1.6, black, ts);
        clipcat::sysfont::draw(&mut pm, "guttural pq jelly 0123456789 ?!@#$%&*()[]{}", 10.0, 86.0, 1.6, black, ts);
        clipcat::sysfont::draw(&mut pm, "작은 크기 한글 테스트 1.2px", 10.0, 110.0, 1.2, black, ts);
        let cut = clipcat::sysfont::truncate_to_width(
            "truncation test: this line is far too long to fit into 200 units of width",
            1.6,
            200.0,
        );
        clipcat::sysfont::draw(&mut pm, &cut, 10.0, 130.0, 1.6, black, ts);
        save(&pm, &dir, "1-text");
    }

    // 2. cat with fish mid-flight (letter badge), mouth opening
    {
        let badge = Badge::from_source(Some("Code"));
        let mut sc = base_scene(Lang::Ko);
        sc.toast = Some(("복사됨! COPIED!", 1.0));
        sc.mouth_open = 0.7;
        sc.fish = Some(FishView {
            x: 165.0,
            y: 80.0,
            rot: 25.0,
            scale: 0.95,
            badge: &badge,
        });
        let mut pm = Pixmap::new(240, 256).unwrap();
        render::render_card(&mut pm, &sc, 1.0);
        save(&pm, &dir, "2-fish");
    }

    // 2b. first-run hotkey hint banner (English + Korean)
    for (lang, hint, name) in [
        (Lang::En, "Clipboard: WIN+SHIFT+V", "2b-hint-en"),
        (Lang::Ko, "클립보드: WIN+SHIFT+V", "2b-hint-ko"),
    ] {
        let mut sc = base_scene(lang);
        sc.hotkey_hint = Some(hint);
        let mut pm = Pixmap::new(240, 256).unwrap();
        render::render_card(&mut pm, &sc, 1.0);
        save(&pm, &dir, name);
    }

    // 3. stats bubble in Korean and English (system font, requirement check)
    for (lang, name) in [(Lang::Ko, "3-bubble-ko"), (Lang::En, "3-bubble-en")] {
        let mut sc = base_scene(lang);
        sc.bubble_alpha = 1.0;
        sc.bubble = Some(BubbleData {
            level: 7,
            pct: 0.62,
            keys: 12345,
            clicks: 987,
            copies: 42,
            minutes: 95,
        });
        let mut pm = Pixmap::new(240, 256).unwrap();
        render::render_card(&mut pm, &sc, 1.0);
        save(&pm, &dir, name);
    }

    // 4. open clipboard panel over the cat: default geometry (Korean +
    //    English) and a moved + enlarged card (resize/move feature)
    {
        let store = demo_store();
        for (lang, capture, source, geometry, name) in [
            (Lang::Ko, true, None, None, "4-panel-ko"),
            (Lang::En, false, None, None, "4-panel-en-paused"),
            (Lang::Ko, true, Some("Code"), None, "4-panel-ko-filtered"),
            (Lang::En, true, None, Some((460.0, 470.0, (-140.0, -380.0))), "4-panel-resized"),
        ] {
            let mut panel = match geometry {
                Some((w, h, off)) => Panel::with_geometry(w, h, off),
                None => Panel::default(),
            };
            panel.toggle();
            panel.source = source.map(str::to_string);
            panel.sel = 1;
            let lt = panel.layout();
            // hover the delete zone of row 2 to show the red delete halo
            panel.cursor = Some((
                lt.row_x + lt.row_w - 8.0,
                lt.rows_y + clipcat::panel::ROW_H * 2.5,
            ));
            panel.clear_armed = !capture; // show the armed (red) clear button too
            let mut sc = base_scene(lang);
            sc.origin = lt.cat;
            let mut pm = Pixmap::new(lt.canvas_w as u32, lt.canvas_h as u32).unwrap();
            render::render_card(&mut pm, &sc, 1.0);
            let view = PanelView {
                panel: &panel,
                store: &store,
                lang,
                capture,
                hint: "WIN+SHIFT+V",
                caret: true,
            };
            render::draw_panel(&mut pm, &view, 1.0);
            save(&pm, &dir, name);
        }
    }

    // 5. the app icon (64 px)
    {
        let mut pm = Pixmap::new(64, 64).unwrap();
        render::draw_icon_scaled(&mut pm, 2.0);
        save(&pm, &dir, "5-icon");
    }
}
