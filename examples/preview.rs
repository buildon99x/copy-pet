//! Dev tool: renders representative frames (cat, fish, panel,
//! Hangul text) to PNGs so changes can be eyeballed without launching the
//! app — useful in headless environments.
//!
//! Usage: cargo run --release --example preview [out_dir]

use clipcat::clipboard::ClipStore;
use clipcat::i18n::Lang;
use clipcat::panel::Panel;
use clipcat::render::{self, Badge, FishView, PanelView, Scene};
use tiny_skia::Pixmap;

fn base_scene(lang: Lang) -> Scene<'static> {
    static ART: std::sync::OnceLock<clipcat::hunter_art::CatArt> = std::sync::OnceLock::new();
    Scene {
        cat_art: ART.get_or_init(|| clipcat::hunter_art::CatArt::load(clipcat::cat::CatKind::Ragdoll).unwrap()),
        hunter_frame: 0,
        capture_paused: false,
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
        particles: &[],
        fish: None,
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

    // Windows image history and inline tag editing, in both panel layouts.
    {
        let mut store = demo_store();
        let mut image = Pixmap::new(240,256).unwrap();
        render::render_card(&mut image, &base_scene(Lang::Ko), 1.0);
        store.add_image(clipcat::clip_image::ClipImage::from_pixmap(&image).unwrap(), Some("SnippingTool".into()));
        let id = store.visible("").iter().find(|c| c.image.is_some()).unwrap().id;
        store.set_tags(id, "디자인, 참고");
        // Keep the image ahead of the demo's pinned text for this preview.
        store.toggle_pin(id);
        for (lang,name) in [(Lang::Ko,"8-images-tags-ko"),(Lang::En,"8-images-tags-en")] {
            let mut panel = Panel::default();
            panel.toggle();
            panel.standalone = true;
            panel.view = 1;
            let l = panel.active_layout();
            let mut pm = Pixmap::new(l.canvas_w as u32,l.canvas_h as u32).unwrap();
            let mut draw = |panel: &Panel, suffix: &str| {
                pm.fill(tiny_skia::Color::TRANSPARENT);
                render::draw_panel(&mut pm, &PanelView { panel, store: &store, lang, capture: true, hint:"WIN+SHIFT+V", caret:true });
                save(&pm,&dir,&format!("{name}{suffix}"));
            };
            draw(&panel, "");
            panel.tag_edit=Some((id,"디자인, 참고".into()));
            draw(&panel, "-editor");
            panel.tag_edit=None;
            panel.expanded=Some(id);
            let l=panel.active_layout();
            panel.cursor=Some((l.row_x+l.row_w-2.5*clipcat::panel::ACT_ZONE,l.rows_y+13.0));
            draw(&panel, "-actions");
            panel.expanded=None;
            panel.cursor=None;
            panel.view=0;
            draw(&panel, "-compact");
            panel.query = "#디자인 #참고".into();
            panel.source = Some("SnippingTool".into());
            draw(&panel, "-search");
        }
    }

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
            render::draw_panel(&mut pm, &view);
            save(&pm, &dir, name);
        }

        // the roomy "thumbnail" card view (two wrapped lines per clip)
        let mut panel = Panel::default();
        panel.toggle();
        panel.view = 1;
        panel.sel = 1;
        let lt = panel.layout();
        let mut sc = base_scene(Lang::Ko);
        sc.origin = lt.cat;
        let mut pm = Pixmap::new(lt.canvas_w as u32, lt.canvas_h as u32).unwrap();
        render::render_card(&mut pm, &sc, 1.0);
        let view = PanelView {
            panel: &panel,
            store: &store,
            lang: Lang::Ko,
            capture: true,
            hint: "WIN+SHIFT+V",
            caret: true,
        };
        render::draw_panel(&mut pm, &view);
        save(&pm, &dir, "4-panel-thumbnail");

        // the caret-anchored flyout: the card alone in its own window (no
        // cat), on a transparent background — what the hotkey opens at the
        // text cursor (Win+V parity). A fresh Pixmap is already transparent.
        let mut panel = Panel::default();
        panel.toggle();
        panel.standalone = true;
        panel.sel = 1;
        let lt = panel.active_layout();
        let mut pm = Pixmap::new(lt.canvas_w as u32, lt.canvas_h as u32).unwrap();
        let view = PanelView {
            panel: &panel,
            store: &store,
            lang: Lang::En,
            capture: true,
            hint: "CTRL+SHIFT+V",
            caret: true,
        };
        render::draw_panel(&mut pm, &view);
        save(&pm, &dir, "4-panel-flyout");

        // the per-row inline actions revealed ("..." / Right arrow): the
        // selected row shows [ paste as text ] [ delete ] on the right (paste
        // button hovered); pin/overflow now live on the right edge too.
        let mut panel = Panel::default();
        panel.toggle();
        panel.sel = 1;
        let lt = panel.layout();
        panel.expanded = Some(panel.visible(&store)[1].id);
        let right = lt.row_x + lt.row_w;
        let y1 = lt.rows_y + clipcat::panel::ROW_H * 1.5;
        panel.cursor = Some((right - clipcat::panel::ACT_ZONE - 4.0, y1));
        let mut sc = base_scene(Lang::En);
        sc.origin = lt.cat;
        let mut pm = Pixmap::new(lt.canvas_w as u32, lt.canvas_h as u32).unwrap();
        render::render_card(&mut pm, &sc, 1.0);
        let view = PanelView {
            panel: &panel,
            store: &store,
            lang: Lang::En,
            capture: true,
            hint: "WIN+SHIFT+V",
            caret: false,
        };
        render::draw_panel(&mut pm, &view);
        save(&pm, &dir, "4-panel-actions");

        // a header-icon tooltip on hover (the clear/trash button)
        let mut panel = Panel::default();
        panel.toggle();
        let lt = panel.layout();
        panel.cursor = Some((
            lt.btn_clear_x + clipcat::panel::BTN / 2.0,
            lt.btn_y + clipcat::panel::BTN / 2.0,
        ));
        let mut sc = base_scene(Lang::Ko);
        sc.origin = lt.cat;
        let mut pm = Pixmap::new(lt.canvas_w as u32, lt.canvas_h as u32).unwrap();
        render::render_card(&mut pm, &sc, 1.0);
        let view = PanelView {
            panel: &panel,
            store: &store,
            lang: Lang::Ko,
            capture: true,
            hint: "WIN+SHIFT+V",
            caret: false,
        };
        render::draw_panel(&mut pm, &view);
        save(&pm, &dir, "4-panel-tooltip");

        // hover a pinned clip's ★ → tooltip teaches the Ctrl/Cmd+P shortcut
        let mut panel = Panel::default();
        panel.toggle();
        let lt = panel.layout();
        let pin_cx = lt.row_x + lt.row_w - clipcat::panel::OVF_ZONE - clipcat::panel::PIN_ZONE / 2.0;
        panel.cursor = Some((pin_cx, lt.rows_y + clipcat::panel::ROW_H * 0.5)); // row 0 (pinned)
        let mut sc = base_scene(Lang::Ko);
        sc.origin = lt.cat;
        let mut pm = Pixmap::new(lt.canvas_w as u32, lt.canvas_h as u32).unwrap();
        render::render_card(&mut pm, &sc, 1.0);
        let view = PanelView {
            panel: &panel,
            store: &store,
            lang: Lang::Ko,
            capture: true,
            hint: "WIN+SHIFT+V",
            caret: false,
        };
        render::draw_panel(&mut pm, &view);
        save(&pm, &dir, "4-panel-pintip");
    }

    // 5. the app icon (64 px)
    {
        let mut pm = Pixmap::new(64, 64).unwrap();
        render::draw_icon_scaled(&mut pm, 2.0);
        save(&pm, &dir, "5-icon");
    }


    // Explicit, local AI request preparation in the existing clipboard panel.
    for lang in [Lang::En, Lang::Ko] {
        use clipcat::ai_workbench::{redact, EditKind, Editor, Favorite, Item, Workbench};
        let mut workbench = Workbench::default();
        workbench.projects[0].name = "ClipCat".into();
        workbench.projects[0].request = if lang == Lang::Ko {
            "{작업 내용}을 구현하세요.\n검증: {검증 명령}"
        } else {
            "Implement {task}.\nValidate: {check}"
        }
        .into();
        workbench.projects[0].tags = vec!["clipcat".into(), "review".into()];
        let template_body = workbench.projects[0].request.clone();
        workbench.projects[0].favorites.push(Favorite {
            name: if lang == Lang::Ko {
                "버그 수정 요청"
            } else {
                "Bug fix request"
            }
            .into(),
            body: template_body,
        });
        workbench.projects[0].items = vec![
            Item {
                text: "fn prepare_request() -> String { /* local only */ }".into(),
                image: false,
                section: 1,
                tags: vec!["code".into()],
            },
            Item {
                text: "Expected: copied text has no API key".into(),
                image: false,
                section: 2,
                tags: vec![],
            },
            Item {
                text: String::new(),
                image: true,
                section: 3,
                tags: vec![],
            },
        ];
        for mode in ["home", "clips", "edit", "review", "review-narrow"] {
            let width=if mode=="review-narrow" {clipcat::panel::MIN_W}else{540.0};
            let mut panel = Panel::with_geometry(width, 480.0, (-56.0, -282.0));
            panel.toggle();
            panel.standalone = true;
            panel.view = 0;
            workbench.scroll = if mode == "clips" { 7 } else { 0 };
            workbench.sel = workbench.scroll;
            workbench.projects[0].editor = if mode == "edit" {
                Some(Editor{kind:EditKind::Request,text:if lang==Lang::Ko {"이 프로젝트의 오류를 수정하세요.\n대상 파일: src/pet.rs\n원본 클립은 보존하세요.\n검증 명령: cargo test --locked --release"}else{"Fix this project's request preparation flow.\nTarget file: src/pet.rs\nKeep original clips intact.\nValidation: cargo test --locked --release"}.into()})
            } else {
                None
            };
            workbench.review = if mode.starts_with("review") {
                Some(redact("## Request\nReview this local example.\n\n## Related code\nAPI_KEY=synthetic-example-token\n\n## Constraints\nKeep originals intact.",lang))
            } else {
                None
            };
            workbench.text_width = panel.active_layout().row_w - 14.0;
            panel.ai_view = Some(workbench.view(lang));
            let lt = panel.active_layout();
            let mut pm = Pixmap::new(lt.canvas_w as u32, lt.canvas_h as u32).unwrap();
            render::draw_panel(
                &mut pm,
                &PanelView {
                    panel: &panel,
                    store: &demo_store(),
                    lang,
                    capture: true,
                    hint: "Ctrl+B",
                    caret: true,
                },
            );
            save(&pm, &dir, &format!("8-ai-{mode}-{}", lang.code()));
        }
    }
    // Default history-card settings must not make AI actions or long drafts inaccessible.
    for lang in [Lang::En, Lang::Ko] {
        for mode in ["compact-home", "draft-top", "draft-end"] {
            use clipcat::ai_workbench::{EditKind, Editor, Workbench};
            let mut panel = Panel::default();
            panel.toggle();
            panel.standalone = true;
            panel.view = 1;
            let mut workbench = Workbench::default();
            workbench.projects[0].name = "ClipCat".into();
            workbench.text_width = panel.active_layout().row_w - 14.0;
            if mode.starts_with("draft") {
                workbench.projects[0].editor = Some(Editor {
                    kind: EditKind::Request,
                    text: (1..=60)
                        .map(|i| {
                            if lang == Lang::Ko {
                                format!("{i:02}번째 요청 초안 줄")
                            } else {
                                format!("Draft line {i:02}")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                });
                if mode == "draft-top" {
                    workbench.nav(clipcat::panel::NavKey::Home, lang, 8);
                }
            }
            panel.ai_view = Some(workbench.view(lang));
            let lt = panel.active_layout();
            let mut pm = Pixmap::new(lt.canvas_w as u32, lt.canvas_h as u32).unwrap();
            render::draw_panel(
                &mut pm,
                &PanelView {
                    panel: &panel,
                    store: &demo_store(),
                    lang,
                    capture: true,
                    hint: "Ctrl+B",
                    caret: true,
                },
            );
            save(&pm, &dir, &format!("9-ai-{mode}-{}", lang.code()));
        }
    }
    // Illustrated ragdoll: every motion frame, all pet sizes, and the persistent
    // pause marker. These are renderer previews, distinct from live captures.
    for frame in 0..16 {
        let mut sc = base_scene(Lang::Ko);
        sc.hunter_frame = frame;
        let mut pm = Pixmap::new(240, 256).unwrap();
        render::render_card(&mut pm, &sc, 1.0);
        save(&pm, &dir, &format!("10-hunter-{frame:02}"));
    }
    for (i, scale) in clipcat::pet::SCALES.iter().enumerate() {
        let mut sc = base_scene(Lang::Ko);
        sc.capture_paused = true;
        let mut pm = Pixmap::new((240.0 * scale).round() as u32, (256.0 * scale).round() as u32).unwrap();
        render::render(&mut pm, &sc, *scale);
        save(&pm, &dir, &format!("10-hunter-scale-{i}"));
        for frame in [0, 6, 15] {
            let mut sc = base_scene(Lang::Ko);
            sc.hunter_frame = frame;
            let mut pm = Pixmap::new((240.0 * scale).round() as u32, (256.0 * scale).round() as u32).unwrap();
            render::render_card(&mut pm, &sc, *scale);
            save(
                &pm,
                &dir,
                &format!("10-hunter-pose-{frame:02}-scale-{i}"),
            );
        }
    }


    // Selected-character gallery and explicit preview states.
    for lang in [Lang::En, Lang::Ko] {
        let mut p = clipcat::pet::Pet::new(clipcat::state::Persist::default());
        p.st.set_lang(lang);
        let opened=std::time::Instant::now();
        p.open_cat_picker();
        println!("gallery open {}: {:.2} ms",lang.code(),opened.elapsed().as_secs_f64()*1000.0);
        let (w,h)=p.canvas_size();
        let mut pm=Pixmap::new(w as u32,h as u32).unwrap();
        p.render_card(&mut pm);
        save(&pm,&dir,&format!("11-cat-picker-{}",lang.code()));
    }
    for kind in clipcat::cat::CatKind::ALL {
        let began=std::time::Instant::now();
        let art=clipcat::hunter_art::CatArt::load(kind).unwrap();
        println!("cat decode {}: {:.2} ms, {} bytes",kind.id(),began.elapsed().as_secs_f64()*1000.0,art.cache_bytes());
        for frame in [0,6,15] {
            let mut sc=base_scene(Lang::En);
            sc.cat_art=&art;
            sc.hunter_frame=frame;
            let mut pm=Pixmap::new(240,256).unwrap();
            render::render_card(&mut pm,&sc,1.0);
            save(&pm,&dir,&format!("12-{}-{frame}",kind.id()));
        }
    }
}
