//! Dev tool: renders representative frames (cat, fish, panel, Hangul text)
//! to PNGs so changes can be eyeballed without launching the app — useful in
//! headless environments and for reviewing the vector-Hangul output.
//!
//! Usage: cargo run --release --example preview [out_dir]

use clipcat::clipboard::ClipStore;
use clipcat::i18n::Lang;
use clipcat::panel::{self, Panel};
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
        lang,
        origin: (0.0, 0.0),
    }
}

fn save(pm: &Pixmap, dir: &str, name: &str) {
    let path = format!("{dir}/{name}.png");
    pm.save_png(&path).unwrap();
    println!("wrote {path}");
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/clipcat-preview".into());
    std::fs::create_dir_all(&dir).unwrap();

    // 1. Hangul + mixed text sample at several sizes
    {
        let mut pm = Pixmap::new(560, 260).unwrap();
        pm.fill(tiny_skia::Color::WHITE);
        let ts = tiny_skia::Transform::identity();
        let black = (40, 40, 40, 255);
        clipcat::font::draw(&mut pm, "클립보드 히스토리 검색...", 10.0, 10.0, 2.0, black, ts);
        clipcat::font::draw(&mut pm, "복사하면 여기에 쌓여요! Copy lands here 123", 10.0, 38.0, 1.6, black, ts);
        clipcat::font::draw(&mut pm, "한글 뷁 쥐 의 꽉 쌍둥이 ㅋㅋ 밝값 흙 — abcXYZ", 10.0, 64.0, 2.0, black, ts);
        clipcat::font::draw(&mut pm, "다람쥐 헌 쳇바퀴에 타고파", 10.0, 92.0, 2.6, black, ts);
        clipcat::font::draw(&mut pm, "레벨 업! 새 아이템: 빨간 목도리", 10.0, 122.0, 2.0, black, ts);
        clipcat::font::draw(&mut pm, "클립 수집 일시정지 / 재개", 10.0, 150.0, 1.6, black, ts);
        clipcat::font::draw(&mut pm, "작은 크기 한글 테스트 1.2px", 10.0, 172.0, 1.2, black, ts);
        clipcat::font::draw(&mut pm, "The quick brown fox jumps over the lazy dog", 10.0, 190.0, 1.6, black, ts);
        clipcat::font::draw(&mut pm, "guttural pq jelly 0123456789 ?!@#$%&*()[]{}", 10.0, 212.0, 1.6, black, ts);
        save(&pm, &dir, "1-hangul");
    }

    // 2. cat with fish mid-flight (letter badge), mouth opening
    {
        let badge = Badge::from_source(Some("Code"));
        let mut sc = base_scene(Lang::Ko);
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

    // 3. stats bubble in Korean and English
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

    // 4. open clipboard panel over the cat (Korean + English)
    {
        let mut store = ClipStore::default();
        store.add_copy("fn main() {\n    println!(\"hello\");\n}".into(), Some("Code".into()));
        store.add_copy("안녕하세요! 클립캣입니다. 이 줄은 길어서 잘리는지 확인합니다 — 아주 아주 길게".into(), Some("브라우저".into()));
        store.add_copy("https://example.com/some/long/path?q=clipboard&lang=ko".into(), Some("chrome".into()));
        store.add_copy("두 번째 클립".into(), None);
        store.add_copy("short".into(), Some("terminal".into()));
        store.add_copy("일곱 번째 줄까지 스크롤 테스트".into(), Some("notes".into()));
        store.add_copy("and one more to overflow the list".into(), Some("mail".into()));
        let pins: Vec<u64> = store.visible("").iter().rev().take(1).map(|c| c.id).collect();
        for id in pins {
            store.toggle_pin(id);
        }

        for (lang, capture, name) in [
            (Lang::Ko, true, "4-panel-ko"),
            (Lang::En, false, "4-panel-en-paused"),
        ] {
            let mut panel = Panel::default();
            panel.toggle();
            panel.sel = 1;
            panel.cursor = Some((150.0, panel::ROWS_Y + panel::ROW_H * 2.5));
            let mut sc = base_scene(lang);
            sc.origin = panel::CAT_ORIGIN;
            let mut pm = Pixmap::new(panel::CANVAS_W as u32, panel::CANVAS_H as u32).unwrap();
            render::render_card(&mut pm, &sc, 1.0);
            let view = PanelView {
                panel: &panel,
                store: &store,
                lang,
                capture,
                hint: "CTRL+SHIFT+V",
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
