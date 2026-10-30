//! Unit tests for [`crate::pet`]. Split out of `pet.rs` to keep the
//! module file focused on the simulation; wired in via `#[path]` (see AGENTS.md).

use super::*;
use crate::panel::PanelAction;

fn pet() -> Pet {
    let mut p = Pet::new(Persist::default());
    p.clips = ClipStore::default(); // don't touch the real config dir
    p
}

#[test]
fn illustrated_ragdoll_has_blue_eyes_and_floats_without_the_old_desk() {
    let mut p = pet();
    p.st.onboarded = true;
    let mut image = Pixmap::new(240, 256).unwrap();
    p.render(&mut image);
    let blue_eyes = (80..185).flat_map(|y| (70..185).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let pixel = image.pixel(x, y).unwrap();
            pixel.alpha() > 200 && pixel.blue() > pixel.red().saturating_add(45)
                && pixel.green() > pixel.red().saturating_add(15)
        }).count();
    assert!(blue_eyes >= 40, "the selected ragdoll needs readable blue eyes; got {blue_eyes} pixels");
    assert_eq!(image.pixel(120, 239).unwrap().alpha(), 0, "pet floats without the old desk");
}

#[test]
fn hunter_idle_stops_redrawing_but_panel_and_copy_changes_still_render() {
    let mut p = pet();
    p.st.onboarded = true;
    assert!(p.advance(), "initial state must draw");
    for _ in 0..10 {
        p.last_tick -= std::time::Duration::from_millis(33);
        assert!(!p.advance(), "a stable illustrated loaf must not repaint");
    }
    p.open_panel();
    assert!(p.advance(), "opening the history must invalidate the frame");
    p.panel_char('x');
    assert!(p.advance(), "search changes must render immediately");
    p.dismiss_panel();
    p.on_copy("new synthetic clip".into(), None, None);
    assert!(p.advance(), "a real copy starts its reaction");
}

#[test]
fn hunter_pause_cancels_copy_feedback_without_removing_collected_clips() {
    let mut p = pet();
    p.on_copy("accepted synthetic clip".into(), None, None);
    p.advance();
    assert!(p.fish.is_some());
    p.run_action(PanelAction::ToggleCapture);
    assert!(p.fish.is_none(), "pause stops a running copy reaction");
    assert!(p.fish_queue.is_empty());
    assert_eq!(p.clips.len(), 1);
}

#[test]
fn hunter_burst_collects_every_clip_but_keeps_only_one_pending_reaction() {
    let mut p = pet();
    p.on_copy("first".into(), None, None);
    p.advance();
    for i in 0..20 { p.on_copy(format!("burst {i}"), None, None); }
    assert_eq!(p.clips.len(), 21);
    assert!(p.fish_queue.len() <= 1, "visual work is coalesced independently of captured clips");
}

#[test]
fn every_cat_fish_reaches_its_authored_mouth_and_crunch_location() {
    let badge = Badge::from_source(None);
    let mut p = pet();
    for kind in crate::cat::CatKind::ALL {
        p.select_cat(kind).unwrap();
        let frame = kind.frame(Some(kind.catch_progress()), 0.0, 0.0, 0.0);
        let (mx,my) = kind.mouth(frame);
        let fish = p.fish_view(&badge,1.0);
        assert!((fish.x-mx).abs()<0.1 && (fish.y-my).abs()<0.1);
        p.fish=Some((Badge::from_source(None),kind.catch_progress()));
        p.nom();
        let heart=p.particles.iter().find(|p| matches!(p.kind,ParticleKind::Heart)).unwrap();
        assert!((heart.x-mx).abs()<0.1 && (heart.y-(my-12.0)).abs()<0.1);
        for progress in [0.0,0.25,0.5,0.75,1.0] {
            assert!(p.fish_view(&badge,progress).y >= my.min(kind.fish_start().1), "fish stays below the mouth");
        }
    }
}

#[test]
fn hunter_hidden_captures_keep_history_without_replaying_visual_backlog() {
    let mut p = pet();
    p.on_copy("before hiding".into(), None, None);
    p.advance();
    p.set_window_level(2);
    p.on_copy("while hidden".into(), None, None);
    assert_eq!(p.clips.len(), 2);
    assert!(p.fish.is_none() && p.fish_queue.is_empty());
    assert!(!p.advance());
    assert!(p.show_window());
    assert!(p.advance());
    assert!(p.fish.is_none());
}

#[test]
fn hunter_deep_sleep_has_no_continuous_particles_or_repaints() {
    let mut p = pet();
    p.st.onboarded = true;
    p.last_event -= std::time::Duration::from_secs(90);
    p.sleep = 1.0;
    p.advance();
    for _ in 0..100 {
        p.last_tick -= std::time::Duration::from_millis(33);
        assert!(!p.advance());
    }
    assert_eq!(p.hunter_frame(p.now_t()), 15);
    assert!(p.particles.is_empty());
}

#[test]
fn every_cat_finishes_its_authored_copy_cycle_with_bounded_visual_work() {
    let mut p=pet();
    p.st.onboarded=true;
    for (i,kind) in crate::cat::CatKind::ALL.into_iter().enumerate() {
        p.select_cat(kind).unwrap();
        p.on_copy(format!("synthetic cycle {i}"),None,None);
        let mut visited=std::collections::BTreeSet::new();
        for _ in 0..((kind.copy_duration()/0.033).ceil() as usize+3) {
            p.last_tick-=std::time::Duration::from_millis(33);
            p.advance();
            visited.insert(p.hunter_frame(p.now_t()));
        }
        assert!((1..12).all(|frame| visited.contains(&frame)), "{kind:?} complete notice/action/recovery: {visited:?}");
        assert!(p.fish.is_none());
        assert_eq!(p.hunter_frame(p.now_t()),kind.frame(None,p.happy,p.sleep,p.blink(p.now_t())));
    }
}

#[test]
fn hunter_tag_selection_invalidates_immediately_without_waiting_for_caret_blink() {
    let mut p = pet();
    p.st.onboarded = true;
    p.clips.add_copy("synthetic tagged clip".into(), None);
    p.open_panel();
    p.panel_nav(NavKey::Tags);
    p.panel.tag_select_all = false;
    p.advance();
    p.panel_nav(NavKey::SelectAll);
    assert!(p.panel.tag_select_all);
    assert!(p.advance(), "Ctrl+A highlight is visible on the next tick");
}

#[test]
fn copy_event_stores_clip_and_queues_fish() {
    let mut p = pet();
    p.on_copy("hello".into(), Some("Code".into()), None);
    assert_eq!(p.clips.len(), 1);
    assert_eq!(p.fish_queue.len(), 1);
    assert_eq!(p.fish_queue[0].letter, 'C');
}

#[test]
fn capture_pause_ignores_copies() {
    let mut p = pet();
    p.st.clip_capture = false;
    p.on_copy("secret".into(), None, None);
    assert!(p.clips.is_empty());
    assert!(p.fish_queue.is_empty());
}

/// Finds the first item (recursing into submenus) carrying `action`.
fn find(entries: &[MenuEntry], action: MenuAction) -> Option<&MenuItem> {
    for e in entries {
        if let MenuEntry::Item(it) = e {
            if it.action == Some(action) {
                return Some(it);
            }
            if let Some(found) = find(&it.submenu, action) {
                return Some(found);
            }
        }
    }
    None
}

#[test]
fn menu_radio_checks_follow_state() {
    let mut p = pet();
    let cur = p.st.scale_idx;
    assert!(find(&p.build_menu("HK", false), MenuAction::SetSize(cur)).unwrap().checked);

    let other = (cur + 1) % 3;
    assert_eq!(p.apply_menu_action(MenuAction::SetSize(other)), MenuOutcome::Handled);
    assert_eq!(p.st.scale_idx, other);
    let m = p.build_menu("HK", false);
    assert!(find(&m, MenuAction::SetSize(other)).unwrap().checked);
    assert!(!find(&m, MenuAction::SetSize(cur)).unwrap().checked);
}

#[test]
fn menu_capture_toggle_marks_paused_and_toasts() {
    let mut p = pet();
    let on = p.st.clip_capture;
    // the item is checked when capture is *paused*
    assert_eq!(
        find(&p.build_menu("HK", false), MenuAction::ToggleCapture).unwrap().checked,
        !on
    );
    assert_eq!(p.apply_menu_action(MenuAction::ToggleCapture), MenuOutcome::Handled);
    assert_eq!(p.st.clip_capture, !on);
    assert!(p.toast.is_some(), "capture toggle shows a toast");
}

#[test]
fn cycle_hotkey_advances_persists_and_toasts() {
    let mut p = pet();
    let before = p.st.hotkey.clone();
    let returned = p.cycle_hotkey();
    assert_eq!(p.st.hotkey, returned, "returns the new spec it persisted");
    assert_ne!(p.st.hotkey, before, "the spec advanced to the next preset");
    assert_eq!(p.st.hotkey, crate::hotkey::PRESETS[1]);
    assert!(p.dirty, "the new spec is persisted");
    assert!(p.toast.is_some(), "the new chord is confirmed via toast");
}

#[test]
fn menu_cycle_hotkey_reregisters_with_new_spec() {
    let mut p = pet();
    // the menu carries a CycleHotkey leaf labelled with the live chord
    let menu = p.build_menu("HK", false);
    let item = find(&menu, MenuAction::CycleHotkey).unwrap();
    assert!(item.label.contains("HK"), "backend live registration label is authoritative");
    let unavailable = p.build_menu("", false);
    assert!(!find(&unavailable, MenuAction::CycleHotkey).unwrap().label.contains(&crate::hotkey::Hotkey::from_spec(&p.st.hotkey).display()));
    // applying it advances the spec and hands the backend the new one to register
    match p.apply_menu_action(MenuAction::CycleHotkey) {
        MenuOutcome::ReregisterHotkey(spec) => assert_eq!(spec, p.st.hotkey),
        other => panic!("expected ReregisterHotkey, got {other:?}"),
    }
}

#[test]
fn paste_on_select_flag_flows_into_pick() {
    let mut p = pet();
    p.st.panel_autoclose = false; // keep the panel state simple
    p.on_copy("hello".into(), None, None);
    let id = p.clips.visible("")[0].id;
    p.toggle_panel();
    // default: copy only, no auto-paste
    let pick = p.run_action(PanelAction::Copy(id)).expect("a pick");
    assert_eq!(pick.text, "hello");
    assert!(!pick.paste, "auto-paste is off by default");
    // turning it on makes the next pick request a paste
    p.toggle_paste_on_select();
    assert!(p.paste_on_select());
    let pick = p.run_action(PanelAction::Copy(id)).expect("a pick");
    assert!(pick.paste, "now the pick also asks the backend to paste");
}

#[test]
fn flyout_pick_always_pastes_for_win_v_parity() {
    let mut p = pet();
    p.st.panel_autoclose = false; // keep the flyout open between picks
    p.on_copy("hello".into(), None, None);
    let id = p.clips.visible("")[0].id;
    p.open_flyout();

    // paste_on_select off (its default; not even exposed on the Windows
    // tray menu) — yet the caret-anchored flyout is the Win+V parity path
    // opened at a focused field, so a pick there still pastes.
    assert!(!p.paste_on_select());
    let pick = p.run_action(PanelAction::Copy(id)).expect("a pick");
    assert!(pick.paste, "flyout pick pastes even with paste_on_select off");

    // turning the setting on must not regress the flyout either
    p.toggle_paste_on_select();
    let pick = p.run_action(PanelAction::Copy(id)).expect("a pick");
    assert!(pick.paste, "flyout pick still pastes with paste_on_select on");
}

#[test]
fn menu_toggle_paste_on_select() {
    let mut p = pet();
    let menu = p.build_menu("HK", false);
    assert!(!find(&menu, MenuAction::TogglePasteOnSelect).unwrap().checked);
    assert_eq!(
        p.apply_menu_action(MenuAction::TogglePasteOnSelect),
        MenuOutcome::Handled
    );
    assert!(p.paste_on_select());
    assert!(p.toast.is_some(), "the toggle is confirmed via toast");
    let menu = p.build_menu("HK", false);
    assert!(find(&menu, MenuAction::TogglePasteOnSelect).unwrap().checked);
}

#[test]
fn menu_autostart_check_reflects_param() {
    let p = pet();
    assert!(find(&p.build_menu("HK", true), MenuAction::ToggleAutostart).unwrap().checked);
    assert!(!find(&p.build_menu("HK", false), MenuAction::ToggleAutostart).unwrap().checked);
    // the toggle is a backend concern (write the LaunchAgent), so it defers
    let mut p = p;
    assert_eq!(p.apply_menu_action(MenuAction::ToggleAutostart), MenuOutcome::ToggleAutostart);
}

#[test]
fn menu_opens_free_cat_gallery() {
    let mut p = pet();
    assert!(
        find(&p.build_menu("HK", false), MenuAction::OpenCatPicker)
            .unwrap()
            .enabled
    );
    p.apply_menu_action(MenuAction::OpenCatPicker);
    assert!(p.cat_picker.is_some());
    assert_eq!(crate::cat::CatKind::ALL.len(), 12);
}

#[test]
fn menu_update_item_appears_only_when_available() {
    let mut p = pet();
    assert!(find(&p.build_menu("HK", false), MenuAction::InstallUpdate).is_none());
    p.notify_update("9.9.9");
    assert!(find(&p.build_menu("HK", false), MenuAction::InstallUpdate).is_some());
    assert_eq!(p.apply_menu_action(MenuAction::InstallUpdate), MenuOutcome::InstallUpdate);
}

#[test]
fn menu_backend_outcomes_do_not_mutate() {
    let mut p = pet();
    assert_eq!(p.apply_menu_action(MenuAction::About), MenuOutcome::ShowAbout);
    assert_eq!(p.apply_menu_action(MenuAction::Quit), MenuOutcome::Quit);
}

#[test]
fn menu_has_a_github_link_under_about() {
    let mut p = pet();
    let m = p.build_menu("HK", false);
    // the GitHub item is present and sits directly after About
    let about = m
        .iter()
        .position(|e| matches!(e, MenuEntry::Item(i) if i.action == Some(MenuAction::About)))
        .unwrap();
    assert!(
        matches!(&m[about + 1], MenuEntry::Item(i) if i.action == Some(MenuAction::OpenGithub)),
        "GitHub should be the item right below About"
    );
    // opening it is OS work handed back to the backend
    assert_eq!(p.apply_menu_action(MenuAction::OpenGithub), MenuOutcome::OpenGithub);
}

#[test]
fn window_level_menu_sets_state_and_restores_on_show() {
    let mut p = pet();
    // default is always-on-top (0): that radio item is checked
    assert!(find(&p.build_menu("HK", false), MenuAction::SetWindowLevel(0)).unwrap().checked);
    // choosing Hide stores the level and asks the backend to apply it
    assert_eq!(
        p.apply_menu_action(MenuAction::SetWindowLevel(2)),
        MenuOutcome::ApplyWindowLevel
    );
    assert_eq!(p.window_level(), 2);
    let m = p.build_menu("HK", false);
    assert!(find(&m, MenuAction::SetWindowLevel(2)).unwrap().checked);
    assert!(!find(&m, MenuAction::SetWindowLevel(0)).unwrap().checked);
    // show_window un-hides, restoring the level in effect before Hide (0)
    assert!(p.show_window());
    assert_eq!(p.window_level(), 0);
    assert!(!p.show_window(), "no-op when already visible");
}

#[test]
fn fish_queue_is_capped() {
    let mut p = pet();
    for i in 0..10 {
        p.on_copy(format!("clip {i}"), None, None);
    }
    assert!(p.fish_queue.len() <= FISH_QUEUE_MAX);
}

#[test]
fn fish_flies_and_gets_eaten() {
    let mut p = pet();
    p.on_copy("fish food".into(), None, None);
    // simulate ~1.5s of ticks
    let mut enjoyed = false;
    for _ in 0..50 {
        p.last_tick -= std::time::Duration::from_millis(33);
        p.advance();
        enjoyed |= p.happy > 0.0;
    }
    assert!(p.fish.is_none());
    assert!(p.fish_queue.is_empty());
    assert!(enjoyed, "nom should make the cat happy during the short reaction");
}

#[test]
fn panel_copy_returns_text_and_closes_panel() {
    let mut p = pet();
    p.on_copy("copy me back".into(), None, None);
    p.toggle_panel();
    assert!(p.panel_open());
    let got = p.panel_nav(NavKey::Enter);
    assert_eq!(got.map(|c| c.text).as_deref(), Some("copy me back"));
    assert!(!p.panel_open(), "picking a clip closes the panel for pasting");
}

#[test]
fn panel_delete_is_undoable_and_keeps_selection_sane() {
    let mut p = pet();
    p.on_copy("one".into(), None, None);
    p.on_copy("two".into(), None, None);
    p.toggle_panel();
    assert_eq!(p.panel_nav(NavKey::Delete), None); // deletes "two"
    assert_eq!(p.clips.len(), 1);
    assert!(p.toast.is_some(), "delete shows the undo hint");
    assert_eq!(p.panel_nav(NavKey::Undo), None);
    assert_eq!(p.clips.len(), 2, "Ctrl+Z restores the clip");
    assert_eq!(p.panel.sel, 0, "selection follows the restored clip");
}

#[test]
fn panel_pin_key_keeps_selection_on_the_clip() {
    let mut p = pet();
    p.on_copy("old".into(), None, None);
    p.on_copy("new".into(), None, None);
    p.toggle_panel();
    p.panel_nav(NavKey::Down); // select "old"
    assert_eq!(p.panel_nav(NavKey::Pin), None);
    let visible = p.panel.visible(&p.clips);
    assert!(visible[p.panel.sel].pinned);
    assert_eq!(visible[p.panel.sel].text, "old", "selection follows the pin");
}

#[test]
fn toggle_panel_changes_canvas_size() {
    let mut p = pet();
    let closed = p.canvas_size();
    p.toggle_panel();
    assert!(p.take_size_changed());
    let open = p.canvas_size();
    assert!(open.0 > closed.0 && open.1 > closed.1);
    // cat-local mapping accounts for the origin shift
    let cat = p.panel.layout().cat;
    let (cx, cy) = p.cat_point(cat.0, cat.1);
    assert_eq!((cx, cy), (0.0, 0.0));
    // the window shifts so the cat itself never moves on screen
    assert_eq!(p.take_window_shift(), (-(cat.0 as i32), -(cat.1 as i32)));
}

#[test]
fn opening_the_panel_requests_a_fit_once() {
    let mut p = pet();
    assert!(!p.take_fit_panel(), "closed panel: nothing to fit");
    p.toggle_panel(); // open
    assert!(p.take_fit_panel(), "opening asks the backend to fit on screen");
    assert!(!p.take_fit_panel(), "drained: only fired once");
    p.toggle_panel(); // close
    assert!(!p.take_fit_panel(), "closing never requests a fit");
}

#[test]
fn open_panel_shows_and_never_closes() {
    let mut p = pet();
    assert!(!p.panel_open());
    p.open_panel(); // closed -> open
    assert!(p.panel_open());
    // pressing the hotkey again keeps it open (the hotkey only ever shows)
    p.open_panel();
    assert!(p.panel_open());
    // an in-flight search survives a re-show (not reset like a fresh open)
    p.panel_char('x');
    assert_eq!(p.panel.query, "x");
    p.open_panel();
    assert_eq!(p.panel.query, "x");
    assert!(p.panel_open());
}

#[test]
fn shift_panel_moves_the_card_without_moving_the_cat() {
    let mut p = pet();
    p.toggle_panel();
    let _ = p.take_size_changed();
    let _ = p.take_window_shift(); // drain the open transition
    let off0 = p.panel.off;
    let anchor0 = p.cat_anchor();

    p.shift_panel(40.0, -30.0);
    assert_eq!(p.panel.off, (off0.0 + 40.0, off0.1 - 30.0), "card slides by the delta");
    assert_eq!((p.st.panel_off_x, p.st.panel_off_y), p.panel.off, "offset is persisted");
    assert!(p.take_size_changed(), "the canvas re-origins around the cat");

    // the window shift exactly cancels the cat's canvas move, so the cat
    // stays put on screen (anchor + shift is invariant)
    let (dx, dy) = p.take_window_shift();
    let anchor1 = p.cat_anchor();
    assert_eq!((anchor1.0 + dx as f32, anchor1.1 + dy as f32), anchor0);

    // a closed panel ignores the shift
    p.toggle_panel();
    let off = p.panel.off;
    p.shift_panel(10.0, 10.0);
    assert_eq!(p.panel.off, off);
}

#[test]
fn panel_keeps_its_size_when_the_cat_scales() {
    let mut p = pet();
    p.toggle_panel();
    assert_eq!(p.scale(), 1.0, "normal is the default size");
    let normal = p.panel.layout();
    // grow the cat to the large size
    p.set_scale_idx(2);
    assert!(p.scale() > 1.0);
    let large = p.panel.layout();
    // the card itself is byte-identical — the panel never scales
    assert_eq!(
        (large.card_x, large.card_y, large.card_w, large.card_h, large.row_w, large.rows),
        (normal.card_x, normal.card_y, normal.card_w, normal.card_h, normal.row_w, normal.rows),
    );
    // ...but the window canvas grows on the cat's side
    assert!(large.canvas_w >= normal.canvas_w && large.canvas_h >= normal.canvas_h);
    assert!(large.canvas_w > normal.canvas_w || large.canvas_h > normal.canvas_h);
}

#[test]
fn drag_pet_moves_the_cat_keeping_the_panel_fixed() {
    let mut p = pet();
    assert!(!p.drag_pet(10.0, 10.0), "closed panel: drag_pet is a no-op");
    p.toggle_panel();
    let _ = p.take_size_changed();
    let _ = p.take_window_shift(); // drain the open transition
    let off0 = p.panel.off;
    let anchor0 = p.panel_anchor();

    // dragging the cat shifts the card offset the opposite way
    assert!(p.drag_pet(-100.0, -320.0));
    assert_eq!(p.panel.off, (off0.0 + 100.0, off0.1 + 320.0));
    assert_eq!((p.st.panel_off_x, p.st.panel_off_y), p.panel.off, "offset persisted");
    assert!(p.take_size_changed());

    // the window shift exactly cancels the card's canvas move, so the card
    // stays pixel-fixed on screen while the cat slides
    let (dx, dy) = p.take_window_shift();
    let anchor1 = p.panel_anchor();
    assert_eq!((anchor1.0 + dx as f32, anchor1.1 + dy as f32), anchor0);
}

#[test]
fn drag_pet_is_a_noop_in_flyout_mode() {
    // In flyout mode `panel.open` is true but the panel lives in its own
    // caret-anchored window and the cat window is cat-only, so dragging the cat
    // must fall through to the backend's whole-window move (return false) —
    // never run the anchored relayout or rewrite the persisted card offset.
    let mut p = pet();
    p.st.panel_autoclose = false;
    p.on_copy("hello".into(), None, None);
    p.open_flyout();
    let off0 = p.panel.off;
    let saved0 = (p.st.panel_off_x, p.st.panel_off_y);
    let _ = p.take_size_changed();
    let _ = p.take_window_shift();

    assert!(!p.drag_pet(-100.0, -320.0), "flyout: drag_pet is a no-op");
    assert_eq!(p.panel.off, off0, "flyout drag must not move the card offset");
    assert_eq!((p.st.panel_off_x, p.st.panel_off_y), saved0, "offset not rewritten");
    assert!(!p.take_size_changed(), "flyout drag must not resize the cat window");
    assert_eq!(p.take_window_shift(), (0, 0), "flyout drag must not shift the window");
}

#[test]
fn lang_toggle_persists_in_state() {
    let mut p = pet();
    let before = p.lang();
    p.toggle_panel();
    let _ = p.run_action(PanelAction::ToggleLang);
    assert_ne!(p.lang(), before);
}

#[test]
fn update_notification_toasts_once_per_version() {
    let mut p = pet();
    assert!(p.update_available().is_none());
    p.notify_update("9.9.9");
    assert_eq!(p.update_available(), Some("9.9.9"));
    assert!(p.toast.is_some());
    p.toast = None;
    p.notify_update("9.9.9"); // same version re-found: stay quiet
    assert!(p.toast.is_none());
    p.notify_update("9.9.10");
    assert_eq!(p.update_available(), Some("9.9.10"));
    assert!(p.toast.is_some());
}

#[test]
fn first_panel_open_marks_onboarded() {
    let mut p = pet();
    assert!(!p.st.onboarded, "starts un-onboarded (first-run hint shown)");
    assert!(p.show_hint());
    p.toggle_panel(); // first open
    assert!(p.st.onboarded, "opening the panel once retires the hint");
    assert!(p.dirty, "the onboarding flag is persisted");
    assert!(!p.show_hint(), "panel open => no under-pet hint");
    p.toggle_panel(); // close again
    assert!(!p.show_hint(), "still onboarded after closing");
}

#[test]
fn render_first_run_hint_smoke() {
    let p = pet(); // default => not onboarded
    assert!(p.show_hint());
    let (w, h) = p.canvas_size();
    let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
    p.render(&mut pm); // draws the hint banner; must not panic
    p.render_card(&mut pm);
    assert!(pm.data().chunks_exact(4).any(|px| px[3] > 0));
}

#[test]
fn render_panel_open_smoke() {
    let mut p = pet();
    p.on_copy("첫 번째 클립".into(), Some("브라우저".into()), None);
    p.on_copy("second clip".into(), Some("Code".into()), None);
    p.toggle_panel();
    let (w, h) = p.canvas_size();
    let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
    p.render(&mut pm); // must not panic
    p.render_card(&mut pm);
    let drawn = pm.data().chunks_exact(4).any(|px| px[3] > 0);
    assert!(drawn);
}

#[test]
fn switching_preserves_selected_cat_and_clears_old_motion() {
    let mut p = pet();
    p.on_copy("synthetic switch".into(), None, None);
    p.advance();
    let chosen = crate::cat::CatKind::ALL[3];
    p.select_cat(chosen).unwrap();
    assert_eq!(p.st.cat_kind, chosen.id());
    assert!(p.fish.is_none() && p.fish_queue.is_empty() && p.particles.is_empty());
    assert_eq!(p.clips.by_index(0).unwrap().text, "synthetic switch");
}

#[test]
fn hover_and_unpaired_release_leave_sleeping_character_quiet() {
    let mut p = pet();
    p.st.onboarded = true;
    p.sleep = 1.0;
    p.last_event -= std::time::Duration::from_secs(90);
    p.advance();
    p.click_bounce(120.0, 180.0);
    p.set_hover(true);
    p.advance();
    assert_eq!(p.hunter_frame(p.now_t()), 15, "input cannot wake the cat");
    assert!(p.particles.is_empty());
    assert_eq!((p.paw_l, p.paw_r, p.rate, p.squash), (0.0, 0.0, 0.0, 0.0));
}

#[test]
fn gallery_navigation_preview_apply_and_history_hotkey_are_explicit() {
    let mut p = pet();
    let size = (p.panel.w, p.panel.h);
    p.open_cat_picker();
    let original = p.cat_kind();
    p.panel_nav(NavKey::Right);
    assert_eq!(p.cat_picker.as_ref().unwrap().selected, crate::cat::CatKind::ALL[1]);
    assert_eq!(p.cat_kind(), original, "browsing does not apply");
    p.panel_char('3');
    assert_eq!(p.cat_picker.as_ref().unwrap().frame(), 15);
    p.panel_nav(NavKey::Enter);
    assert_eq!(p.cat_kind(), crate::cat::CatKind::ALL[1]);
    assert!(p.cat_picker.is_none());
    assert_eq!((p.panel.w, p.panel.h), size);
    p.open_cat_picker();
    p.open_panel();
    assert!(p.cat_picker.is_none(), "history shortcut reaches history from gallery");
    assert!(p.panel_open());
}

#[test]
fn copy_wakes_a_sleeping_cat_immediately_but_input_is_quiet() {
    let mut p = pet();
    p.sleep = 1.0;
    p.last_event -= std::time::Duration::from_secs(90);
    p.advance();
    assert_eq!(p.sleep,1.0);
    assert!(p.particles.is_empty() && p.toast.is_none());
    p.on_copy("wake synthetic".into(),None,None);
    assert_eq!(p.sleep,0.0);
}

#[test]
fn pause_after_catch_cancels_particles_and_compression_without_replay() {
    let mut p = pet();
    p.on_copy("synthetic caught copy".into(), None, None);
    p.advance();
    for _ in 0..40 {
        p.last_tick -= std::time::Duration::from_millis(33);
        p.advance();
        if !p.particles.is_empty() { break; }
    }
    assert!(!p.particles.is_empty() && p.squash > 0.0, "reach the actual catch effects first");
    p.run_action(PanelAction::ToggleCapture);
    assert!(p.fish.is_none() && p.fish_queue.is_empty());
    assert!(p.particles.is_empty(), "pause cancels already-spawned copy particles");
    assert_eq!((p.squash, p.happy), (0.0, 0.0));
    p.run_action(PanelAction::ToggleCapture);
    p.advance();
    assert!(p.fish.is_none() && p.fish_queue.is_empty() && p.particles.is_empty());
    assert_eq!((p.squash, p.happy), (0.0, 0.0));
    assert_eq!(p.clips.len(), 1);
}

#[test]
fn hide_after_catch_cancels_particles_and_compression_before_quick_show() {
    let mut p = pet();
    p.on_copy("synthetic caught hidden copy".into(), None, None);
    p.advance();
    for _ in 0..40 {
        p.last_tick -= std::time::Duration::from_millis(33);
        p.advance();
        if !p.particles.is_empty() { break; }
    }
    assert!(!p.particles.is_empty() && p.squash > 0.0, "reach the actual catch effects first");
    p.set_window_level(2);
    assert!(p.fish.is_none() && p.fish_queue.is_empty());
    assert!(p.particles.is_empty(), "hide cancels already-spawned copy particles");
    assert_eq!((p.squash, p.happy), (0.0, 0.0));
    assert!(p.show_window());
    p.advance();
    assert!(p.fish.is_none() && p.fish_queue.is_empty() && p.particles.is_empty());
    assert_eq!((p.squash, p.happy), (0.0, 0.0));
    assert_eq!(p.clips.len(), 1);
}

#[test]
fn fish_approach_stays_below_each_current_pose_mouth_for_every_cat() {
    let badge = Badge::from_source(None);
    let mut p = pet();
    for kind in crate::cat::CatKind::ALL {
        p.select_cat(kind).unwrap();
        for sample in 0..=100 {
            let ft = sample as f32 / 100.0;
            let progress = ft * kind.catch_progress();
            let frame = kind.frame(Some(progress), 0.0, 0.0, 0.0);
            let (_, mouth_y) = kind.mouth(frame);
            let fish = p.fish_view(&badge, ft);
            assert!(fish.y + 0.01 >= mouth_y,
                "{kind:?} approach {ft:.2}, frame {frame}: fish y {} crosses above current mouth y {mouth_y}", fish.y);
        }
        let catch_frame = kind.frame(Some(kind.catch_progress()), 0.0, 0.0, 0.0);
        let endpoint = kind.mouth(catch_frame);
        let fish = p.fish_view(&badge, 1.0);
        assert!((fish.x - endpoint.0).abs() < 0.01 && (fish.y - endpoint.1).abs() < 0.01,
            "the approach still finishes exactly at the authored capture mouth");
    }
}

#[test]
fn retina_flyout_renders_the_whole_logical_card_at_native_density() {
    let mut p = pet();
    p.open_flyout();
    let (w, h) = p.flyout_size();
    let mut pm = Pixmap::new(w as u32 * 2, h as u32 * 2).unwrap();
    p.render_flyout_at_density(&mut pm, 2.0);
    let l = p.panel.layout_standalone();
    let x = ((l.card_x + l.card_w - 12.0) * 2.0) as u32;
    let y = ((l.card_y + 60.0) * 2.0) as u32;
    assert!(pm.pixel(x, y).unwrap().alpha() > 200, "2x panel must cover its physical right edge");
}

#[test]
fn retina_panel_text_is_rasterized_in_device_pixels() {
    let mut p = pet();
    p.st.set_lang(crate::i18n::Lang::En);
    p.open_flyout();
    let (w, h) = p.flyout_size();
    let mut pm = Pixmap::new(w as u32 * 2, h as u32 * 2).unwrap();
    p.render_flyout_at_density(&mut pm, 2.0);
    // Only header text, away from rounded edges, fish and button vectors.
    // A nearest-neighbour enlargement of a 1x glyph repeats every 2x2 block.
    let mut detailed = 0;
    for y in (26..56).step_by(2) {
        for x in (72..280).step_by(2) {
            let a = pm.pixel(x, y).unwrap();
            if [pm.pixel(x + 1, y), pm.pixel(x, y + 1), pm.pixel(x + 1, y + 1)]
                .into_iter().any(|b| b.unwrap() != a) { detailed += 1; }
        }
    }
    assert!(detailed > 40, "glyph edges need native 2x detail: {detailed}");
}

#[test]
fn retina_pointer_hits_and_resize_preserve_logical_panel_geometry() {
    let mut p = pet();
    p.open_flyout();
    let d = crate::viewport::Density::new(2.0);
    let l = p.panel.layout_standalone();
    let (x, y) = d.logical(((l.card_x + 100.0) as f64 * 2.0, (l.card_y + 100.0) as f64 * 2.0));
    assert!(p.panel_hit(x, y));
    let (gx, gy) = d.logical((((l.card_x + l.card_w - 4.0) * 2.0) as f64,
        ((l.card_y + l.card_h - 4.0) * 2.0) as f64));
    assert!(p.panel_drag_start(gx, gy));
    let (dx, dy) = d.logical((80.0, 48.0));
    p.panel_drag_update(dx, dy);
    p.panel_drag_end();
    assert_eq!(p.panel.w, l.card_w + 40.0);
    assert_eq!(p.panel.h, l.card_h + 24.0);
    assert_eq!((p.st.panel_w, p.st.panel_h), (p.panel.w, p.panel.h));
    let (w, h) = p.flyout_size();
    let mut normal = Pixmap::new(w as u32, h as u32).unwrap();
    let mut explicit = normal.clone();
    p.render_flyout(&mut normal);
    p.render_flyout_at_density(&mut explicit, 1.0);
    assert_eq!(normal.data(), explicit.data(), "1x backend pixels stay identical");
}

#[test]
fn legacy_input_and_copy_preserve_settings_without_growth_effects() {
    let state: Persist = serde_json::from_str(r#"{"total_xp":279,"cat_kind":"british-shorthair","sound_mode":0,"onboarded":true}"#).unwrap();
    let mut p = Pet::new(state);
    p.clips = ClipStore::default();
    let before = serde_json::to_value(&p.st).unwrap();
    p.advance();
    assert_eq!(serde_json::to_value(&p.st).unwrap(), before, "global activity cannot change persisted state");
    p.on_copy("synthetic private companion".into(), None, None);
    assert_eq!(p.clips.by_index(0).unwrap().text, "synthetic private companion");
    assert_eq!(serde_json::to_value(&p.st).unwrap(), before, "copy history has no growth state");
    assert!(p.toast.is_none() && p.particles.is_empty(), "no level-up effects on copy");
    assert_eq!(p.st.cat_kind, "british-shorthair");
}

fn body_point(p: &Pet) -> (f32, f32) {
    (0..256).flat_map(|y| (0..240).map(move |x| (x as f32, y as f32)))
        .find(|&(x,y)| p.cat_hit(x,y)).unwrap()
}

#[test]
fn local_affection_requires_silhouette_press_release_and_no_logical_drag() {
    let mut p = pet();
    let (x,y) = body_point(&p);
    assert!(!p.cat_press(0., 0.));
    assert!(!p.cat_release(x,y));
    assert!(p.cat_press(x,y));
    assert!(!p.cat_release(0.,0.));
    p.st.locked = true;
    assert!(p.cat_press(x,y));
    p.cat_motion(x+4., y);
    assert!(!p.cat_release(x,y), "returning after a drag is not a click, even locked");
    assert!(p.cat_press(x,y));
    assert!(p.cat_release(x,y));
    assert!(p.affection.is_some());
    p.open_panel();
    let layout = p.panel.layout();
    assert!(!p.cat_press(layout.card_x + 20., layout.card_y + 20.));
}

#[test]
fn affection_is_bounded_yields_at_copy_boundary_and_clears_on_hide() {
    let mut p = pet();
    p.sleep = 1.;
    p.on_copy("synthetic affection concurrency".into(),None,None);
    p.advance();
    p.pet();
    for _ in 0..100 { p.pet(); }
    assert_eq!(p.clips.len(), 1);
    assert!(p.affection_pending);
    assert!(p.affection.is_none(), "running copy reaches a safe end first");
    assert_eq!(p.sleep, 0.);
    for _ in 0..14 { p.last_tick -= std::time::Duration::from_millis(100); p.advance(); }
    assert!(p.affection.is_some());
    assert!(!p.affection_pending);
    p.run_action(PanelAction::ToggleCapture);
    p.pet();
    assert!(p.affection.is_some(), "pause still permits intentional affection");
    p.set_window_level(2);
    assert!(p.affection.is_none());
    assert!(!p.affection_pending);
    p.show_window();
    p.blink_next = f32::MAX;
    p.advance();
    assert!(!p.advance(), "settled hidden effects do not repaint");
}

#[test]
fn affection_finishes_one_pending_response_and_copy_saves_during_it() {
    let mut p = pet();
    p.blink_next = f32::MAX;
    p.pet();
    for _ in 0..100 { p.pet(); }
    p.on_copy("saved during affection".into(), None, None);
    assert_eq!(p.clips.len(),1);
    assert!(p.fish.is_none());
    for _ in 0..30 { p.last_tick -= std::time::Duration::from_millis(100); p.advance(); }
    assert!(p.affection.is_none() && !p.affection_pending);
    assert!(p.fish.is_some(), "deferred copy decorates after bounded affection");
    for _ in 0..40 { p.last_tick -= std::time::Duration::from_millis(100); p.advance(); }
    assert!(!p.advance(), "completed affection and copy settle without repaint");
}
