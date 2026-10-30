//! End-to-end tests of the platform-agnostic core: drive the public [`Pet`]
//! API exactly the way the backends do (copy events from the clipboard
//! listener, the hotkey toggling the panel, mouse clicks at panel
//! coordinates, typed search characters) and assert on what the user gets
//! back: clipboard text, filtered rows, rendered pixels.
//!
//! What cannot run headless (real Win32 hotkey registration, OS clipboard,
//! system tray) is covered by the unit-tested seams these tests drive.

use clipcat::clipboard::ClipStore;
use clipcat::hotkey::{self, Hotkey};
use clipcat::i18n::Lang;
use clipcat::menu::{MenuAction, MenuEntry, MenuItem, MenuOutcome};
use clipcat::panel::{self, NavKey};
use clipcat::pet::Pet;
use clipcat::state::Persist;
use tiny_skia::Pixmap;

fn pet() -> Pet {
    let mut p = Pet::new(Persist::default());
    p.clips = ClipStore::default(); // keep tests off the real config dir
    p
}

#[test]
fn legacy_accessory_is_ignored_without_changing_the_selected_cat() {
    let state: Persist=serde_json::from_str(r#"{"accessory":2,"total_xp":7890,"total_keys":42}"#).unwrap();
    let p=Pet::new(state);
    assert_eq!(p.cat_kind(),clipcat::cat::CatKind::Ragdoll);
    assert!(!serde_json::to_string(&p.st).unwrap().contains("accessory"));
}

/// First menu item (recursing into submenus) carrying `action`.
fn menu_find(entries: &[MenuEntry], action: MenuAction) -> Option<&MenuItem> {
    for e in entries {
        if let MenuEntry::Item(it) = e {
            if it.action == Some(action) {
                return Some(it);
            }
            if let Some(found) = menu_find(&it.submenu, action) {
                return Some(found);
            }
        }
    }
    None
}

fn copy(p: &mut Pet, text: &str, source: Option<&str>) {
    p.on_copy(text.to_string(), source.map(str::to_string), None);
}

/// The headline flow: copies arrive from several apps, the panel opens (as
/// the global hotkey does), Tab narrows to one app, Enter hands the picked
/// clip's text back for the OS clipboard and closes the panel for pasting;
/// reopened, Esc unwinds filter then panel.
#[test]
fn copy_filter_and_copy_back_flow() {
    let mut p = pet();
    copy(&mut p, "breaking news headline", Some("Chrome"));
    copy(&mut p, "fn main() {}", Some("Code"));
    copy(&mut p, "plain note", None);
    assert_eq!(p.clips.len(), 3);

    p.toggle_panel(); // = WM_HOTKEY / middle-click
    assert!(p.panel_open());
    assert!(p.take_size_changed(), "backends resize to the panel canvas");

    // Tab cycles the source filter: most recent app first
    assert_eq!(p.panel_nav(NavKey::Tab), None);
    assert_eq!(p.panel.source.as_deref(), Some("Code"));
    let text = p.panel_nav(NavKey::Enter);
    assert_eq!(text.map(|c| c.text).as_deref(), Some("fn main() {}"));
    assert!(!p.panel_open(), "picking a clip closes the panel for pasting");
    assert!(p.take_size_changed(), "backends shrink back to the cat canvas");

    // reopening starts fresh (no filter), Tab twice reaches the older app
    p.toggle_panel();
    assert_eq!(p.panel.source, None);
    p.panel_nav(NavKey::Tab);
    p.panel_nav(NavKey::Tab);
    assert_eq!(p.panel.source.as_deref(), Some("Chrome"));
    let text = p.panel_nav(NavKey::Enter);
    assert_eq!(text.map(|c| c.text).as_deref(), Some("breaking news headline"));

    // Esc: filter first, panel second
    p.toggle_panel();
    p.panel_nav(NavKey::Tab);
    assert!(p.panel.source.is_some());
    assert_eq!(p.panel_nav(NavKey::Esc), None);
    assert_eq!(p.panel.source, None);
    assert!(p.panel_open());
    assert_eq!(p.panel_nav(NavKey::Esc), None);
    assert!(!p.panel_open(), "second Esc closes the panel");
}

/// The hotkey flyout: opens the panel in its own caret-anchored window
/// without moving the cat. The cat window keeps its plain size and is told
/// nothing changed, while the flyout reports its own card-sized canvas; a
/// pick still hands back the clip text and dismisses the flyout.
#[test]
fn hotkey_flyout_leaves_the_cat_window_untouched() {
    let mut p = pet();
    copy(&mut p, "from chrome", Some("Chrome"));
    copy(&mut p, "from code", Some("Code"));

    let cat_size = p.canvas_size(); // cat-only, panel closed
    let _ = p.take_size_changed(); // drain anything from setup/copies

    p.open_flyout(); // = WM_HOTKEY on the native backend
    assert!(p.flyout_open());
    assert!(p.panel_open());
    assert!(
        !p.take_size_changed(),
        "the cat window must not resize for the flyout"
    );
    assert_eq!(
        p.canvas_size(),
        cat_size,
        "the cat window keeps its plain cat-only size while the flyout is up"
    );

    // the flyout window is just the card plus margins, not the union canvas
    assert_eq!(
        p.flyout_size(),
        (panel::DEFAULT_W as i32 + 8, panel::DEFAULT_H as i32 + 8)
    );

    // a pick still returns the clip text and (autoclose) dismisses the flyout,
    // again without ever touching the cat window
    let pick = p.panel_nav(NavKey::Enter);
    assert_eq!(pick.map(|c| c.text).as_deref(), Some("from code"));
    assert!(!p.flyout_open(), "autoclose dismisses the flyout");
    assert!(!p.panel_open());
    assert!(
        !p.take_size_changed(),
        "dismissing the flyout never resizes the cat window"
    );
}

/// The embedded (middle-click) panel and the flyout must be distinguishable so
/// the Windows backend only makes the cat window focusable for the *embedded*
/// panel — not the flyout (which is its own window). Pressing the hotkey while
/// the embedded panel is up retracts it and hands ownership to the flyout.
#[test]
fn embedded_vs_flyout_panel_are_distinct() {
    let mut p = pet();
    copy(&mut p, "a clip", Some("Code"));

    // middle-click: the panel is embedded in the cat window (it grows + focuses)
    p.toggle_panel();
    assert!(p.embedded_panel_open(), "middle-click opens the embedded panel");
    assert!(!p.flyout_open());

    // hotkey while embedded is up: retract the embedded panel, open the flyout
    p.open_flyout();
    assert!(p.flyout_open(), "hotkey hands ownership to the flyout");
    assert!(
        !p.embedded_panel_open(),
        "the cat window is no longer the embedded panel (so it must not stay focusable)"
    );

    // closing the flyout leaves neither mode active
    p.close_flyout();
    assert!(!p.flyout_open());
    assert!(!p.embedded_panel_open());
    assert!(!p.panel_open());
}

/// The same filter driven by the mouse, through real panel coordinates —
/// the path both backends' click handlers take.
#[test]
fn filter_button_and_row_clicks() {
    let mut p = pet();
    copy(&mut p, "older from chrome", Some("Chrome"));
    copy(&mut p, "newest from code", Some("Code"));
    p.toggle_panel();
    let lt = p.panel.layout();

    // funnel header button engages the filter (pure panel state, no copy)
    let got = p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(got, None);
    assert_eq!(p.panel.source.as_deref(), Some("Code"));

    // the second row is empty under the filter -> a click there is a no-op
    // (use the layout's actual row height so this holds in either view mode)
    let empty_y = lt.rows_y + lt.row_h * 1.5;
    assert_eq!(p.panel_click(150.0, empty_y), None);

    // cycle past the last source: back to all apps, both rows clickable
    p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(p.panel.source.as_deref(), Some("Chrome"));
    p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(p.panel.source, None);
    assert_eq!(p.panel_click(150.0, empty_y).map(|c| c.text).as_deref(), Some("older from chrome"));
    assert!(!p.panel_open(), "a row click copies and closes the panel");

    // reopened, filtering again and clicking the first row copies it back
    p.toggle_panel();
    p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(p.panel.source.as_deref(), Some("Code"));
    let row_y = lt.rows_y + lt.row_h / 2.0;
    let text = p.panel_click(150.0, row_y);
    assert_eq!(text.map(|c| c.text).as_deref(), Some("newest from code"));
}

/// Deleting is forgiving end-to-end: Del removes the selected clip, Ctrl+Z
/// brings it back, and the clear-all button needs a confirming second press.
#[test]
fn delete_undo_and_two_step_clear_flow() {
    let mut p = pet();
    copy(&mut p, "keep me pinned", Some("Code"));
    copy(&mut p, "fat finger victim", Some("Chrome"));
    copy(&mut p, "latest", None);
    p.toggle_panel();

    // pin the oldest via the keyboard (End + Ctrl+P), selection follows it
    p.panel_nav(NavKey::End);
    assert_eq!(p.panel_nav(NavKey::Pin), None);
    let visible = p.panel.visible(&p.clips);
    assert_eq!(visible[p.panel.sel].text, "keep me pinned");
    assert!(visible[p.panel.sel].pinned, "pinned clips sort first");
    assert_eq!(p.panel.sel, 0);

    // Del removes the selected clip; Ctrl+Z restores it
    p.panel_nav(NavKey::Down); // select "latest"
    assert_eq!(p.panel_nav(NavKey::Delete), None);
    assert_eq!(p.clips.len(), 2);
    assert!(p.clips.visible("latest").is_empty());
    assert_eq!(p.panel_nav(NavKey::Undo), None);
    assert_eq!(p.clips.len(), 3, "Ctrl+Z restores the deleted clip");

    // clear-all: first click only arms (nothing deleted), second clears
    let lt = p.panel.layout();
    let (bx, by) = (lt.btn_clear_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(p.panel_click(bx, by), None);
    assert_eq!(p.clips.len(), 3, "first press arms, deletes nothing");
    assert!(p.panel.clear_armed);
    assert_eq!(p.panel_click(bx, by), None);
    assert_eq!(p.clips.len(), 1, "second press clears the unpinned clips");
    assert_eq!(p.clips.pinned_count(), 1, "pinned clip survives the clear");

    // even a full clear is undoable as one operation
    assert_eq!(p.panel_nav(NavKey::Undo), None);
    assert_eq!(p.clips.len(), 3);
}

/// Search text (typed or IME-committed) combines with the source filter.
#[test]
fn search_combines_with_source_filter() {
    let mut p = pet();
    copy(&mut p, "안녕하세요 from chrome", Some("Chrome"));
    copy(&mut p, "hello from code", Some("Code"));
    copy(&mut p, "안녕 from code", Some("Code"));
    p.toggle_panel();

    p.panel_nav(NavKey::Tab); // filter: Code
    for c in "안녕".chars() {
        p.panel_char(c);
    }
    let text = p.panel_nav(NavKey::Enter);
    assert_eq!(text.map(|c| c.text).as_deref(), Some("안녕 from code"));

    // dropping the filter widens the same query to Chrome's clip too
    let visible = p.panel.visible(&p.clips);
    assert_eq!(visible.len(), 1);
    p.panel.source = None;
    let visible = p.panel.visible(&p.clips);
    assert_eq!(visible.len(), 2);
}

/// Rendering e2e: panel open with an active filter renders (system font or
/// pixel-font fallback) into the exact canvas the backends present.
#[test]
fn render_filtered_panel_end_to_end() {
    let mut p = pet();
    copy(&mut p, "클립 하나", Some("브라우저"));
    copy(&mut p, "clip two", Some("Code"));
    p.set_panel_hint(Hotkey::from_spec(hotkey::DEFAULT).display());
    p.toggle_panel();
    p.panel_nav(NavKey::Tab);
    assert!(p.panel.source.is_some());

    let (w, h) = p.canvas_size();
    assert_eq!((w, h), (360, 542), "default panel canvas at scale 1.0");
    let lt = p.panel.layout();
    assert_eq!((lt.canvas_w as i32, lt.canvas_h as i32), (w, h));
    let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
    p.render(&mut pm); // native path (transparent)
    let drawn = pm.data().chunks_exact(4).filter(|px| px[3] > 0).count();
    assert!(drawn > 1000, "panel + cat must rasterize ({drawn} px)");
    p.render_card(&mut pm); // portable path (opaque card)
    let opaque = pm.data().chunks_exact(4).all(|px| px[3] == 255);
    assert!(opaque, "card render fills the canvas");
}

/// Hotkey configuration e2e: default spec, persistence round-trip, custom
/// values and the reset of hand-edited garbage.
#[test]
fn hotkey_spec_lifecycle() {
    let st = Persist::default();
    assert_eq!(st.hotkey, hotkey::DEFAULT);
    let hk = Hotkey::from_spec(&st.hotkey);
    assert!(hk.win && hk.shift && !hk.ctrl && !hk.alt);
    // the `win` modifier is the OS super key: WIN+...+V / CMD+...+V / SUPER+...+V
    let expect = format!("{}+SHIFT+V", hotkey::super_name());
    assert_eq!((hk.key, hk.display()), ('V', expect));

    // round-trips through the state.json representation
    let json = serde_json::to_string(&st).unwrap();
    assert!(json.contains("\"hotkey\":\"win+shift+v\""));
    let back: Persist = serde_json::from_str(&json).unwrap();
    assert_eq!(back.hotkey, hotkey::DEFAULT);

    // a user-customized combination is honored...
    let custom = Hotkey::from_spec("ctrl+alt+c");
    assert!(custom.ctrl && custom.alt && !custom.win && !custom.shift);
    assert_eq!(custom.display(), "CTRL+ALT+C");
    // ...and garbage falls back to the default instead of losing the hotkey
    assert_eq!(Hotkey::from_spec("not-a-hotkey"), hk);
    // the registration fallback combo stays valid
    assert_eq!(Hotkey::from_spec(hotkey::FALLBACK).display(), "CTRL+SHIFT+V");
}

/// The panel footer hint follows whatever the backend actually registered.
#[test]
fn panel_hint_follows_registered_hotkey() {
    let mut p = pet();
    copy(&mut p, "x", None);
    for hint in ["WIN+SHIFT+V", "CTRL+SHIFT+V", ""] {
        p.set_panel_hint(hint.to_string());
        if !p.panel_open() {
            p.toggle_panel();
        }
        let (w, h) = p.canvas_size();
        let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
        p.render(&mut pm); // must not panic for any hint incl. empty
    }
}

/// The context menu is a real tree (submenus + separators) and every settings
/// item drives the same state the user sees — checks follow state, and the
/// destructive/OS items defer to the backend.
#[test]
fn context_menu_drives_settings_end_to_end() {
    let mut p = pet();

    let m = p.build_menu("CMD+SHIFT+V", false);
    assert!(m.iter().any(|e| matches!(e, MenuEntry::Separator)), "has separators");
    // the clipboard entry shows the live hotkey label, like the Windows tray
    assert!(menu_find(&m, MenuAction::TogglePanel).unwrap().label.contains("CMD+SHIFT+V"));
    // size is a 3-way radio submenu
    for i in 0..3 {
        assert!(menu_find(&m, MenuAction::SetSize(i)).is_some());
    }

    // sound: toggle and watch the radio check move
    assert_eq!(p.apply_menu_action(MenuAction::SetSound(2)), MenuOutcome::Handled);
    assert_eq!(p.st.sound_mode, 1);
    assert!(menu_find(&p.build_menu("HK", false), MenuAction::SetSound(1)).unwrap().checked);
    assert!(!menu_find(&p.build_menu("HK", false), MenuAction::SetSound(0)).unwrap().checked);

    // lock / auto-update toggles flip the persisted flags
    let (lock0, au0) = (p.st.locked, p.st.auto_update);
    p.apply_menu_action(MenuAction::ToggleLock);
    p.apply_menu_action(MenuAction::ToggleAutoUpdate);
    assert_eq!((p.st.locked, p.st.auto_update), (!lock0, !au0));

    // language submenu switches the language
    let lang0 = p.lang();
    let other = if lang0 == Lang::En { Lang::Ko } else { Lang::En };
    p.apply_menu_action(MenuAction::SetLang(other));
    assert_eq!(p.lang(), other);

    // panel opens through the menu exactly like the hotkey/middle-click
    assert!(!p.panel_open());
    p.apply_menu_action(MenuAction::TogglePanel);
    assert!(p.panel_open());

    // about/quit are backend outcomes
    assert_eq!(p.apply_menu_action(MenuAction::About), MenuOutcome::ShowAbout);
    assert_eq!(p.apply_menu_action(MenuAction::Quit), MenuOutcome::Quit);
}

/// Panel resize/move end-to-end: drags through the public Pet API (the path
/// both backends' mouse handlers take), geometry persistence, and the
/// window-shift contract that keeps the cat anchored on screen.
#[test]
fn panel_resize_and_move_persist_and_anchor() {
    let mut p = pet();
    // this test pins the compact-list geometry (8 rows by default, 34px rows so
    // +68px == 2 rows); the panel now opens in card view, so switch to the list.
    if p.panel.view != 0 {
        p.toggle_panel_view();
    }
    for i in 0..15 {
        copy(&mut p, &format!("clip {i}"), None);
    }
    p.toggle_panel();
    assert!(p.take_size_changed());
    // opening grows the canvas around the cat; the window shifts so the
    // cat itself does not move on screen (default layout: cat at (60,286))
    assert_eq!(p.take_window_shift(), (-60, -286));

    let l0 = p.panel.layout();
    assert_eq!(l0.rows, 8);

    // resize drag from the bottom-right grip: +60 wide, +68 tall = 2 rows
    let (gx, gy) = (l0.card_x + l0.card_w - 4.0, l0.card_y + l0.card_h - 4.0);
    assert!(p.panel_drag_start(gx, gy));
    assert!(p.panel_dragging());
    p.panel_drag_update(60.0, 68.0);
    p.panel_drag_end();
    let l1 = p.panel.layout();
    assert_eq!((l1.card_w, l1.card_h), (l0.card_w + 60.0, l0.card_h + 68.0));
    assert_eq!(l1.rows, 10, "a taller card shows more clips");
    assert!(p.take_size_changed());
    // growing right/down leaves the cat's corner alone: no window shift
    assert_eq!(p.take_window_shift(), (0, 0));
    // the new size is persisted state
    assert_eq!((p.st.panel_w, p.st.panel_h), (l1.card_w, l1.card_h));

    // move drag from the header strip: 30 left, 20 up — only the card moves
    assert!(p.panel_drag_start(l1.card_x + 60.0, l1.card_y + 10.0));
    p.panel_drag_update(-30.0, -20.0);
    p.panel_drag_end();
    assert_eq!(
        (p.st.panel_off_x, p.st.panel_off_y),
        (panel::DEFAULT_OFF.0 - 30.0, panel::DEFAULT_OFF.1 - 20.0)
    );
    let l2 = p.panel.layout();
    assert_eq!(l2.cat, (90.0, 306.0), "canvas re-origins around the card");
    assert!(p.take_size_changed());
    assert_eq!(p.take_window_shift(), (-30, -20), "window shift keeps the cat put");

    // rows/buttons/search never start a drag (clicks still work there)
    assert!(!p.panel_drag_start(l2.row_x + 10.0, l2.rows_y + 10.0));
    assert!(!p.panel_drag_start(l2.btn_close_x + 8.0, l2.btn_y + 8.0));

    // reopening keeps the user's geometry; render at the new size is sane
    p.toggle_panel();
    p.toggle_panel();
    assert_eq!(p.panel.layout().card_w, l1.card_w);
    let (w, h) = p.canvas_size();
    let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
    p.render(&mut pm);

    // geometry round-trips through state.json
    let json = serde_json::to_string(&p.st).unwrap();
    let back: Persist = serde_json::from_str(&json).unwrap();
    assert_eq!((back.panel_w, back.panel_h), (l1.card_w, l1.card_h));
    assert_eq!(back.panel_off_x, p.st.panel_off_x);
}

/// Quick-copy hotkeys (Ctrl+0..9): 0 copies the top clip, 9 the tenth, and
/// the mapping follows whatever filter/search the panel currently shows —
/// exactly the rows wearing the digit badges.
#[test]
fn quick_copy_hotkeys_copy_top_clips() {
    let mut p = pet();
    for i in 0..12 {
        let src = if i % 2 == 0 { "Code" } else { "Chrome" };
        copy(&mut p, &format!("clip {i}"), Some(src));
    }
    p.toggle_panel();
    assert_eq!(p.panel_nav(NavKey::Quick(0)).map(|c| c.text).as_deref(), Some("clip 11"));
    assert!(!p.panel_open(), "quick copy closes the panel like Enter");

    p.toggle_panel();
    assert_eq!(p.panel_nav(NavKey::Quick(9)).map(|c| c.text).as_deref(), Some("clip 2"));

    // respects the source filter (most recent app first: Chrome)
    p.toggle_panel();
    p.panel_nav(NavKey::Tab);
    assert_eq!(p.panel.source.as_deref(), Some("Chrome"));
    assert_eq!(p.panel_nav(NavKey::Quick(1)).map(|c| c.text).as_deref(), Some("clip 9"));

    // respects the search query
    p.toggle_panel();
    for c in "clip 1".chars() {
        p.panel_char(c);
    }
    // matches "clip 11", "clip 10", "clip 1" (newest first)
    assert_eq!(p.panel_nav(NavKey::Quick(2)).map(|c| c.text).as_deref(), Some("clip 1"));

    // out of range: nothing copied, panel stays open
    p.toggle_panel();
    for c in "clip 3".chars() {
        p.panel_char(c);
    }
    assert_eq!(p.panel_nav(NavKey::Quick(5)), None);
    assert!(p.panel_open());
}

/// The auto-close-after-copy setting end-to-end: on by default, toggled
/// through the shared menu model (tray/NSMenu/shortcut all call the same
/// action), honored by every copy path, persisted in state.json.
#[test]
fn panel_autoclose_toggle_keeps_panel_open() {
    let mut p = pet();
    copy(&mut p, "first", None);
    copy(&mut p, "second", None);
    assert!(p.st.panel_autoclose, "closes after copy by default");

    let m = p.build_menu("HK", false);
    assert!(menu_find(&m, MenuAction::TogglePanelAutoClose).unwrap().checked);
    assert_eq!(
        p.apply_menu_action(MenuAction::TogglePanelAutoClose),
        MenuOutcome::Handled
    );
    assert!(!p.st.panel_autoclose);
    assert!(!menu_find(&p.build_menu("HK", false), MenuAction::TogglePanelAutoClose)
        .unwrap()
        .checked);

    // with auto-close off, Enter / quick keys / row clicks keep the panel up
    p.toggle_panel();
    assert_eq!(p.panel_nav(NavKey::Enter).map(|c| c.text).as_deref(), Some("second"));
    assert!(p.panel_open(), "panel stays open for more copies");
    assert_eq!(p.panel_nav(NavKey::Quick(1)).map(|c| c.text).as_deref(), Some("first"));
    assert!(p.panel_open());
    let lt = p.panel.layout();
    let row_y = lt.rows_y + panel::ROW_H / 2.0;
    assert_eq!(p.panel_click(150.0, row_y).map(|c| c.text).as_deref(), Some("second"));
    assert!(p.panel_open());

    // flipping it back restores the close-on-copy behavior
    p.apply_menu_action(MenuAction::TogglePanelAutoClose);
    assert!(p.panel_nav(NavKey::Enter).is_some());
    assert!(!p.panel_open());

    // the setting round-trips through state.json
    let json = serde_json::to_string(&p.st).unwrap();
    let back: Persist = serde_json::from_str(&json).unwrap();
    assert!(back.panel_autoclose);
}

/// Holding a key must not inflate the stats: the input gate counts a key
/// once per physical press, exactly what both backends' hooks feed it.
#[test]
fn held_key_counts_once_through_the_input_gate() {
    use clipcat::input;
    let _ = input::drain();
    input::key_down(0x41);
    input::key_down(0x41); // OS auto-repeat while held
    input::key_down(0x41);
    input::key_down(0x42); // another key pressed alongside
    let (k, _, _) = input::drain();
    assert_eq!(k, 2, "auto-repeat does not count");
    input::key_up(0x41);
    input::key_down(0x41); // released and pressed again: counts
    assert_eq!(input::drain().0, 1);
    input::key_up(0x41);
    input::key_up(0x42);
}

/// Every gallery card is free, previews do not apply, and Apply persists stable IDs.
#[test]
fn context_menu_gallery_applies_all_twelve_cats() {
    let mut p=pet();
    for (i,kind) in clipcat::cat::CatKind::ALL.into_iter().enumerate() {
        let previous=p.cat_kind();
        assert!(menu_find(&p.build_menu("HK",false),MenuAction::OpenCatPicker).unwrap().enabled);
        p.apply_menu_action(MenuAction::OpenCatPicker);
        let l=p.panel.layout();
        p.panel_click(l.card_x+20.0+(i%4) as f32*144.0,l.card_y+60.0+(i/4) as f32*108.0);
        assert_eq!(p.cat_kind(),previous,"browsing only previews");
        for key in ['1','2','3'] { p.panel_char(key); }
        p.panel_click(l.card_x+380.0,l.card_y+540.0);
        assert_eq!(p.cat_kind(),kind);
        let restored: Persist=serde_json::from_str(&serde_json::to_string(&p.st).unwrap()).unwrap();
        assert_eq!(Pet::new(restored).cat_kind(),kind);
    }
}

/// Rich-format paste parity (ADR-0014): a normal pick re-emits the original
/// formatting, while the per-row "paste as text" action strips it. Drives the
/// panel exactly as the backends do (overflow click reveals the row actions).
#[test]
fn copy_preserves_formats_but_paste_as_text_strips_them() {
    use clipcat::clipboard::RichFormats;
    let mut p = pet();
    p.on_copy_rich(
        "rich clip".into(),
        Some("Code".into()),
        None,
        Some(RichFormats { html: Some("<b>rich clip</b>".into()), rtf_b64: None }),
    );
    p.toggle_panel();
    let l = p.panel.active_layout();
    let right = l.row_x + l.row_w;
    let y0 = l.rows_y + panel::ROW_H / 2.0;

    // the default pick (Enter) preserves the original formatting
    let copied = p.panel_nav(NavKey::Enter).expect("a clip pick");
    assert_eq!(copied.text, "rich clip");
    assert!(!copied.plain_only);
    assert_eq!(copied.formats.unwrap().html.as_deref(), Some("<b>rich clip</b>"));

    // reopen, reveal the row actions, and "paste as text" strips formatting
    p.toggle_panel();
    assert_eq!(p.panel_click(right - 4.0, y0), None, "overflow only reveals actions");
    let plain = p.panel_click(right - panel::ACT_ZONE - 4.0, y0).expect("a clip pick");
    assert_eq!(plain.text, "rich clip");
    assert!(plain.plain_only && plain.paste, "paste as text: plain + explicit paste");
    assert!(plain.formats.is_none());
}

/// Mouse-free "paste as text" (ADR-0014): Ctrl/Cmd+Enter on the selected clip
/// strips formatting and pastes it, without opening the "..." action menu.
#[test]
fn ctrl_enter_pastes_selected_clip_as_text() {
    use clipcat::clipboard::RichFormats;
    let mut p = pet();
    p.on_copy_rich(
        "rich clip".into(),
        Some("Code".into()),
        None,
        Some(RichFormats { html: Some("<b>rich clip</b>".into()), rtf_b64: None }),
    );
    p.toggle_panel();
    let pick = p.panel_nav(NavKey::PasteText).expect("a clip pick");
    assert_eq!(pick.text, "rich clip");
    assert!(pick.plain_only && pick.paste, "plain text, explicitly pasted");
    assert!(pick.formats.is_none());
}

#[test]
fn edit_tags_by_mouse_and_keyboard_then_filter() {
    let mut p = pet();
    copy(&mut p, "tagged note", None);
    p.toggle_panel();
    let id = p.clips.visible("")[0].id;
    let l = p.panel.active_layout();
    // The overflow reveals a # tag editor reachable on both panel layouts.
    p.panel_click(l.row_x+l.row_w-4.0, l.rows_y+10.0);
    p.panel_click(l.row_x+l.row_w-2.5*panel::ACT_ZONE, l.rows_y+10.0);
    assert!(p.panel.tag_edit.is_some());
    for c in "업무, Work".chars() { p.panel_char(c); }
    assert!(p.panel_nav(NavKey::Enter).is_none(), "saving tags never copies or pastes");
    assert!(p.panel.tag_edit.is_none());
    assert!(p.panel_open());
    assert_eq!(p.clips.get(id).unwrap().tags, ["업무", "Work"]);
    p.panel_nav(NavKey::Tags);
    p.panel_char('x');
    p.panel_nav(NavKey::Esc);
    assert_eq!(p.clips.get(id).unwrap().tags, ["업무", "Work"], "Esc discards draft");
    p.panel_nav(NavKey::Tags);
    p.panel_nav(NavKey::SelectAll);
    for c in "work, 참고".chars() { p.panel_char(c); }
    p.panel_nav(NavKey::Enter);
    assert_eq!(p.clips.get(id).unwrap().tags, ["work", "참고"]);
    p.panel_nav(NavKey::Tags);
    p.panel_nav(NavKey::SelectAll);
    for c in "a b c d e f g h i".chars() { p.panel_char(c); }
    p.panel_nav(NavKey::Enter);
    assert!(p.panel.tag_edit.is_some(), "invalid draft stays editable");
    assert_eq!(p.clips.get(id).unwrap().tags, ["work", "참고"]);
    p.panel_nav(NavKey::Esc);
    for c in "#work".chars() { p.panel_char(c); }
    assert_eq!(p.panel.visible(&p.clips).len(), 1);
    assert_eq!(p.panel_nav(NavKey::Enter).unwrap().text, "tagged note");
}

#[test]
fn image_pick_preserves_pixels_and_flyout_paste_intent() {
    use clipcat::clip_image::ClipImage;
    for flyout in [false,true] {
        let mut p = pet();
        let mut pm = Pixmap::new(20,10).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(30,60,90,255));
        let image = ClipImage::from_pixmap(&pm).unwrap();
        p.on_copy_image(image.clone(), Some("Paint".into()), None);
        if flyout { p.open_flyout(); } else { p.toggle_panel(); }
        assert!(p.panel_nav(NavKey::PasteText).is_none(), "images never paste an empty text label");
        assert!(p.panel_open());
        let (w,h) = if flyout { p.flyout_size() } else { p.canvas_size() };
        let mut rendered = Pixmap::new(w as u32,h as u32).unwrap();
        if flyout { p.render_flyout(&mut rendered); } else { p.render_card(&mut rendered); }
        assert!(rendered.data().iter().any(|&b| b != 0));
        let pick = p.panel_nav(NavKey::Enter).unwrap();
        assert_eq!(pick.image, Some(image));
        assert!(pick.text.is_empty());
        assert_eq!(pick.paste, flyout);
        assert!(!pick.plain_only);
    }
}

#[test]
fn paused_capture_does_not_store_images() {
    let mut p = pet();
    p.st.clip_capture = false;
    let image = clipcat::clip_image::ClipImage::from_pixmap(&Pixmap::new(1,1).unwrap()).unwrap();
    p.on_copy_image(image, None, None);
    assert!(p.clips.is_empty());
}

/// Saving while a tag filter is active must refresh cached rows immediately,
/// including removing the selected row and clamping selection before a pick.
#[test]
fn tag_search_cache_refreshes_after_edit_in_both_panel_layouts() {
    for flyout in [false, true] {
        let mut p = pet();
        copy(&mut p, "분기 보고서", Some("Paint"));
        let text_id = p.clips.visible("")[0].id;
        let image = clipcat::clip_image::ClipImage::from_pixmap(&Pixmap::new(2, 2).unwrap()).unwrap();
        p.on_copy_image(image.clone(), Some("Paint".into()), None);
        let image_id = p.clips.visible("")[0].id;
        assert!(p.clips.set_tags(text_id, "업무 Work"));
        assert!(p.clips.set_tags(image_id, "업무 Work"));
        assert!(p.clips.visible_filtered("wor 업 PAINT", Some("paint")).iter().any(|c| c.id == image_id),
            "images are searchable by partial tags and source without body text");
        assert!(p.clips.visible("#Paint").is_empty(), "an app name is not an exact tag");
        copy(&mut p, "다른 앱 보고서", Some("Code"));
        let other_id = p.clips.visible("")[0].id;
        assert!(p.clips.set_tags(other_id, "업무 Work"));
        if flyout { p.open_flyout(); } else { p.toggle_panel(); }
        p.panel_nav(NavKey::Tab); // Code
        p.panel_nav(NavKey::Tab); // Paint
        assert_eq!(p.panel.source.as_deref(), Some("Paint"));
        for c in "#업무 #wOrK".chars() { p.panel_char(c); }
        assert_eq!(p.panel.visible(&p.clips).iter().map(|c| c.id).collect::<Vec<_>>(), [image_id, text_id]);
        p.panel_nav(NavKey::Down);
        p.panel_nav(NavKey::Tags);
        p.panel_nav(NavKey::SelectAll);
        for c in "보관".chars() { p.panel_char(c); }
        assert!(p.panel_nav(NavKey::Enter).is_none());
        assert!(p.panel_open());
        assert_eq!(p.panel.query, "#업무 #wOrK", "editing preserves the active query");
        assert_eq!(p.panel.visible(&p.clips).iter().map(|c| c.id).collect::<Vec<_>>(), [image_id]);
        assert_eq!(p.panel.sel, 0, "selection clamps when the edited clip no longer matches");
        let pick = p.panel_nav(NavKey::Enter).unwrap();
        assert_eq!(pick.image, Some(image));
        assert_eq!(pick.paste, flyout);
    }
}

// Workbench rows are driven through exactly the public panel API used by both
// native and portable backends; no clipboard writes occur until confirmation.
fn ai_action(p: &mut Pet, prefix: &str) -> Option<clipcat::pet::ClipPick> {
    let view = p.workbench.view(Lang::En);
    let index = view
        .rows
        .iter()
        .position(|s| s.starts_with(prefix))
        .unwrap();
    p.workbench.sel = index;
    p.panel_nav(NavKey::Enter)
}

#[test]
fn ai_workbench_public_flow_collects_reorders_fills_and_confirms_a_safe_copy() {
    let mut p = pet();
    p.st.lang = "en".into();
    p.workbench = clipcat::ai_workbench::Workbench::default();
    copy(&mut p, "example source", None);
    copy(&mut p, "API_KEY=synthetic-example", None);
    p.apply_menu_action(MenuAction::OpenWorkbench);
    assert!(p.panel_open());
    assert!(p.panel.ai_view.is_some());
    ai_action(&mut p, "Request:");
    assert!(p.workbench_editing());
    p.workbench_paste("Implement {task}\nRun {check}");
    p.panel_nav(NavKey::Enter);
    ai_action(&mut p, "★ Save request");
    p.workbench_paste("Implementation");
    p.panel_nav(NavKey::Enter);
    ai_action(&mut p, "Fill variable {task}");
    p.workbench_paste("the feature");
    p.panel_nav(NavKey::Enter);
    ai_action(&mut p, "Fill variable {check}");
    p.workbench_paste("cargo test");
    p.panel_nav(NavKey::Enter);
    ai_action(&mut p, "+ Select clips");
    assert!(p.panel.ai_adding);
    // Selecting in collection mode adds snapshots and cannot paste a secret.
    assert!(p.panel_nav(NavKey::Enter).is_none());
    p.panel_nav(NavKey::Down);
    assert!(p.panel_nav(NavKey::Enter).is_none());
    p.panel_nav(NavKey::Workbench);
    assert_eq!(p.workbench.project().items.len(), 2);
    ai_action(&mut p, "2. [Related code]");
    assert_eq!(p.workbench.project().items[1].section, 2);
    assert!(ai_action(&mut p, "Review &").is_none());
    let review = p.workbench.review.as_ref().unwrap();
    assert_eq!(review.findings.len(), 1);
    assert!(!review.text.contains("synthetic-example"));
    let pick = ai_action(&mut p, "Confirm:").unwrap();
    assert!(!pick.paste);
    assert!(pick.plain_only);
    assert!(pick.formats.is_none());
    assert!(pick.text.contains("Implement the feature"));
    assert!(pick.text.contains("cargo test"));
    assert!(pick.text.contains("[REDACTED_1]"));
    assert!(p.clips.visible_filtered("synthetic-example", None).len() == 1);
}

#[test]
fn workbench_keeps_history_sensitive_text_pick_and_flyout_intent_unchanged() {
    let mut p = pet();
    p.workbench = clipcat::ai_workbench::Workbench::default();
    p.workbench.projects[0].request = "existing draft".into();
    copy(&mut p, "Authorization: Bearer synthetic0123456789", None);
    p.open_flyout();
    let pick = p.panel_nav(NavKey::Enter).unwrap();
    assert!(pick.paste);
    assert_eq!(pick.text, "Authorization: Bearer synthetic0123456789");
    assert_eq!(p.workbench.project().request, "existing draft");
    assert!(p.workbench.review.is_none());
}

#[test]
fn workbench_editor_paste_is_scoped_and_drafts_restore_after_project_switch() {
    let mut p = pet();
    p.st.lang = "en".into();
    p.workbench = clipcat::ai_workbench::Workbench::default();
    p.workbench_paste("ignored");
    assert!(p.workbench.project().request.is_empty());
    p.open_workbench();
    ai_action(&mut p, "Request:");
    p.workbench_paste("first");
    p.panel_nav(NavKey::SelectAll);
    p.workbench_paste("한글\nreplacement");
    // Header P button remains reachable while an editor is open.
    let lt = p.panel.active_layout();
    p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert!(!p.workbench_editing());
    ai_action(&mut p, "Create project");
    ai_action(&mut p, "Request:");
    p.workbench_paste("project two");
    p.panel_nav(NavKey::Enter);
    ai_action(&mut p, "Switch project:");
    ai_action(&mut p, "Default");
    assert!(p.workbench_editing());
    assert_eq!(
        p.workbench.project().editor.as_ref().unwrap().text,
        "한글\nreplacement"
    );
    p.panel_nav(NavKey::Enter);
    assert_eq!(p.workbench.project().request, "한글\nreplacement");
}

#[test]
fn hidden_workbench_editor_does_not_accept_an_explicit_paste() {
    let mut p = pet();
    p.st.lang = "en".into();
    p.workbench = clipcat::ai_workbench::Workbench::default();
    p.open_workbench();
    ai_action(&mut p, "Request:");
    p.workbench_paste("retained");
    p.toggle_panel();
    assert!(!p.panel_open());
    assert!(!p.workbench_editing());
    p.workbench_paste("must be ignored");
    assert_eq!(
        p.workbench.project().editor.as_ref().unwrap().text,
        "retained"
    );
}

#[test]
fn resizing_embedded_workbench_rewraps_the_complete_review() {
    let mut p = pet();
    p.st.lang = "en".into();
    p.workbench = clipcat::ai_workbench::Workbench::default();
    p.open_workbench();
    ai_action(&mut p, "Request:");
    p.workbench_paste(&"한글 검토 문구입니다. ".repeat(30));
    p.panel_nav(NavKey::Enter);
    ai_action(&mut p, "Review &");
    let before = p.panel.ai_view.as_ref().unwrap().rows.len();
    let lt = p.panel.active_layout();
    assert!(p.panel_drag_start(lt.card_x + lt.card_w - 3.0, lt.card_y + lt.card_h - 3.0));
    p.panel_drag_update(clipcat::panel::MIN_W - lt.card_w, 0.0);
    p.panel_drag_end();
    let view = p.panel.ai_view.as_ref().unwrap();
    assert!(view.rows.len() > before);
    let max = p.panel.active_layout().row_w - 14.0;
    let first = view.rows.iter().position(|s| s == "← Back").unwrap() + 1;
    assert!(view.rows[first..]
        .iter()
        .all(|s| clipcat::sysfont::measure(s, clipcat::ai_workbench::BODY_FONT) <= max));
    assert!(view.rows[first..]
        .concat()
        .contains(&"한글 검토 문구입니다. ".repeat(29)));
}

#[test]
fn workbench_uses_compact_rows_without_changing_history_card_preferences() {
    for standalone in [false, true] {
        for history_view in [0, 1] {
            let mut p = pet();
            p.st.lang = "en".into();
            p.workbench = clipcat::ai_workbench::Workbench::default();
            p.panel.view = history_view;
            p.st.panel_view = history_view;
            if standalone {
                p.open_flyout();
            }
            p.open_workbench();
            let lt = p.panel.active_layout();
            assert!(lt.rows >= 7);
            assert_eq!(lt.row_h, clipcat::ai_workbench::ROW_HEIGHT);
            assert!(p.panel.ai_view.as_ref().unwrap().rows[..lt.rows]
                .iter()
                .any(|s| s.starts_with("Review &")));
            p.panel_nav(NavKey::Workbench);
            assert_eq!(p.panel.view, history_view);
            assert_eq!(p.st.panel_view, history_view);
            assert_eq!(
                p.panel.active_layout().row_h,
                if history_view == 1 {
                    clipcat::panel::ROW_H_THUMB
                } else {
                    clipcat::panel::ROW_H
                }
            );
        }
    }
}

#[test]
fn long_workbench_draft_is_readable_through_public_wheel_and_navigation() {
    let mut p = pet();
    p.st.lang = "en".into();
    p.workbench = clipcat::ai_workbench::Workbench::default();
    p.open_workbench();
    ai_action(&mut p, "Request:");
    p.workbench_paste(
        &(1..=60)
            .map(|i| format!("draft {i:02}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let visible = |p: &Pet| {
        let view = p.panel.ai_view.as_ref().unwrap();
        let start = view.visible_start(p.panel.active_layout().rows);
        view.rows[start].clone()
    };
    let end_start = visible(&p);
    p.panel_wheel(-8);
    assert_ne!(visible(&p), end_start);
    p.panel_nav(NavKey::Home);
    assert_eq!(visible(&p), "draft 01");
    p.panel_nav(NavKey::PageDown);
    assert_ne!(visible(&p), "draft 01");
    p.panel_nav(NavKey::End);
    assert_eq!(visible(&p), end_start);
    p.panel_nav(NavKey::Home);
    p.workbench_paste("\nnew end");
    let view = p.panel.ai_view.as_ref().unwrap();
    assert!(view.follow_tail);
    assert_eq!(view.rows.last().unwrap(), "new end");
    p.panel_nav(NavKey::Workbench);
    p.panel_nav(NavKey::Workbench);
    assert_eq!(
        p.workbench
            .project()
            .editor
            .as_ref()
            .unwrap()
            .text
            .lines()
            .count(),
        61
    );
}

#[test]
fn workbench_header_only_responds_inside_each_visible_button() {
    let mut p = pet();
    p.st.lang = "en".into();
    p.workbench = clipcat::ai_workbench::Workbench::default();
    p.open_workbench();
    let lt = p.panel.active_layout();
    let home = p.panel.ai_view.as_ref().unwrap().rows.clone();
    for x in [
        lt.card_x - 10.0,
        lt.btn_view_x + 5.0,
        lt.btn_pause_x + 5.0,
        lt.btn_close_x + clipcat::panel::BTN + 3.0,
        lt.card_x + lt.card_w + 10.0,
    ] {
        p.panel_click(x, lt.btn_y + 8.0);
        assert!(p.panel_open());
        assert_eq!(p.lang(), Lang::En);
        assert_eq!(p.panel.ai_view.as_ref().unwrap().rows, home);
    }
    p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert!(p
        .panel
        .ai_view
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .any(|s| s == "Create project"));
    p.panel_click(lt.btn_lang_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(p.lang(), Lang::Ko);
    p.panel_click(lt.btn_close_x + 8.0, lt.btn_y + 8.0);
    assert!(!p.panel_open());
}

#[test]
fn workbench_footer_hit_targets_match_the_visible_text_and_card_bounds() {
    let mut p = pet();
    p.st.lang = "en".into();
    p.workbench = clipcat::ai_workbench::Workbench::default();
    p.toggle_panel();
    let lt = p.panel.active_layout();
    p.panel_click(lt.card_x + lt.card_w + 10.0, lt.footer_y + 5.0);
    assert!(!p.workbench.open);
    p.panel_click(lt.row_x, lt.footer_y + 5.0);
    assert!(!p.workbench.open);
    let entry = p.panel.workbench_footer_text(Lang::En);
    let width = clipcat::sysfont::measure(&entry, 1.4);
    p.panel_click(
        lt.card_x + lt.card_w - 10.0 - width / 2.0,
        lt.footer_y + 5.0,
    );
    assert!(p.workbench.open);
    p.panel_click(lt.row_x + lt.row_w - 50.0, lt.footer_y + 5.0);
    assert!(p.workbench.open);
    p.panel_click(lt.row_x + 5.0, lt.card_y + lt.card_h + 5.0);
    assert!(p.workbench.open);
    p.panel_click(lt.row_x + 5.0, lt.footer_y + 5.0);
    assert!(!p.workbench.open);
}
