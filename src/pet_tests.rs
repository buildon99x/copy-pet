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
fn copy_event_stores_clip_grants_xp_and_queues_fish() {
    let mut p = pet();
    let xp0 = p.st.total_xp;
    p.on_copy("hello".into(), Some("Code".into()), None);
    assert_eq!(p.clips.len(), 1);
    assert_eq!(p.st.total_xp, xp0 + XP_PER_COPY);
    assert_eq!(p.st.copies_today, 1);
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
    assert_eq!(p.st.total_xp, 0);
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
    assert!(item.label.contains(&crate::hotkey::Hotkey::from_spec(&p.st.hotkey).display()));
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
fn menu_locks_accessories_until_their_level() {
    let mut p = pet(); // level 1: every accessory locked
    let m = p.build_menu("HK", false);
    assert!(find(&m, MenuAction::SetAccessory(0)).unwrap().enabled, "None always enabled");
    assert!(!find(&m, MenuAction::SetAccessory(1)).unwrap().enabled, "locked accessory greyed");
    // applying a locked accessory is a guarded no-op
    p.apply_menu_action(MenuAction::SetAccessory(1));
    assert_eq!(p.st.accessory, 0);
}

#[test]
fn menu_reset_waits_for_confirmation() {
    let mut p = pet();
    p.st.total_keys = 99;
    p.st.total_xp = 500;
    assert_eq!(p.apply_menu_action(MenuAction::ResetStats), MenuOutcome::ConfirmReset);
    assert_eq!(p.st.total_keys, 99, "ResetStats does not reset until the backend confirms");
    p.reset_stats();
    assert_eq!(p.st.total_keys, 0);
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
    for _ in 0..50 {
        p.last_tick -= std::time::Duration::from_millis(33);
        p.advance(0, 0, 0);
    }
    assert!(p.fish.is_none());
    assert!(p.fish_queue.is_empty());
    assert!(p.happy > 0.0, "nom should make the cat happy");
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
fn body_click_levels_up_immediately() {
    let mut p = pet();
    // one XP below the level-2 threshold; a body click grants +1 XP, which
    // must trigger the level-up in-call rather than deferring to a later tick.
    p.st.total_xp = crate::state::xp_to_next(1) - 1;
    assert_eq!(p.level(), 1);
    p.click_bounce(0.0, 0.0);
    assert_eq!(p.level(), 2, "body click XP must level up within click_bounce");
}

#[test]
fn multi_level_jump_equips_highest_unlocked_accessory() {
    let mut p = pet();
    // A single large XP grant jumps many levels at once. The auto-equip must
    // land on the highest newly-unlocked accessory, not silently skip it when
    // the destination level does not exactly equal an accessory unlock level.
    p.st.total_xp = 10_000_000;
    p.maybe_level_up();
    let cur = p.level();
    let expected = crate::state::ACCESSORIES
        .iter()
        .enumerate()
        .filter(|(_, a)| a.level <= cur)
        .max_by_key(|(_, a)| a.level)
        .map(|(i, _)| i + 1)
        .expect("some accessory is unlocked at a high level");
    assert_eq!(
        p.st.accessory, expected,
        "a multi-level jump must equip the highest newly-unlocked accessory"
    );
}
