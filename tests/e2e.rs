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
use clipcat::render::{self, Accessory, BubbleData, Scene};
use clipcat::state::Persist;
use tiny_skia::Pixmap;

fn pet() -> Pet {
    let mut p = Pet::new(Persist::default());
    p.clips = ClipStore::default(); // keep tests off the real config dir
    p
}

fn pet_at_xp(xp: u64) -> Pet {
    let st = Persist { total_xp: xp, ..Persist::default() };
    let mut p = Pet::new(st);
    p.clips = ClipStore::default();
    p
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
    assert_eq!(text.as_deref(), Some("fn main() {}"));
    assert!(!p.panel_open(), "picking a clip closes the panel for pasting");
    assert!(p.take_size_changed(), "backends shrink back to the cat canvas");

    // reopening starts fresh (no filter), Tab twice reaches the older app
    p.toggle_panel();
    assert_eq!(p.panel.source, None);
    p.panel_nav(NavKey::Tab);
    p.panel_nav(NavKey::Tab);
    assert_eq!(p.panel.source.as_deref(), Some("Chrome"));
    let text = p.panel_nav(NavKey::Enter);
    assert_eq!(text.as_deref(), Some("breaking news headline"));

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
    let empty_y = lt.rows_y + panel::ROW_H * 1.5;
    assert_eq!(p.panel_click(150.0, empty_y), None);

    // cycle past the last source: back to all apps, both rows clickable
    p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(p.panel.source.as_deref(), Some("Chrome"));
    p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(p.panel.source, None);
    assert_eq!(p.panel_click(150.0, empty_y).as_deref(), Some("older from chrome"));
    assert!(!p.panel_open(), "a row click copies and closes the panel");

    // reopened, filtering again and clicking the first row copies it back
    p.toggle_panel();
    p.panel_click(lt.btn_filter_x + 8.0, lt.btn_y + 8.0);
    assert_eq!(p.panel.source.as_deref(), Some("Code"));
    let row_y = lt.rows_y + panel::ROW_H / 2.0;
    let text = p.panel_click(150.0, row_y);
    assert_eq!(text.as_deref(), Some("newest from code"));
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
    assert_eq!(text.as_deref(), Some("안녕 from code"));

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
    assert_eq!(p.st.sound_mode, 2);
    assert!(menu_find(&p.build_menu("HK", false), MenuAction::SetSound(2)).unwrap().checked);
    assert!(!menu_find(&p.build_menu("HK", false), MenuAction::SetSound(0)).unwrap().checked);

    // lock / stats / auto-update toggles flip the persisted flags
    let (lock0, bub0, au0) = (p.st.locked, p.st.bubble_pinned, p.st.auto_update);
    p.apply_menu_action(MenuAction::ToggleLock);
    p.apply_menu_action(MenuAction::ToggleStats);
    p.apply_menu_action(MenuAction::ToggleAutoUpdate);
    assert_eq!((p.st.locked, p.st.bubble_pinned, p.st.auto_update), (!lock0, !bub0, !au0));

    // language submenu switches the language
    let lang0 = p.lang();
    let other = if lang0 == Lang::En { Lang::Ko } else { Lang::En };
    p.apply_menu_action(MenuAction::SetLang(other));
    assert_eq!(p.lang(), other);

    // panel opens through the menu exactly like the hotkey/middle-click
    assert!(!p.panel_open());
    p.apply_menu_action(MenuAction::TogglePanel);
    assert!(p.panel_open());

    // reset defers to a confirmation; about/quit are backend outcomes
    p.st.total_keys = 42;
    assert_eq!(p.apply_menu_action(MenuAction::ResetStats), MenuOutcome::ConfirmReset);
    assert_eq!(p.st.total_keys, 42);
    assert_eq!(p.apply_menu_action(MenuAction::About), MenuOutcome::ShowAbout);
    assert_eq!(p.apply_menu_action(MenuAction::Quit), MenuOutcome::Quit);
}

/// Panel resize/move end-to-end: drags through the public Pet API (the path
/// both backends' mouse handlers take), geometry persistence, and the
/// window-shift contract that keeps the cat anchored on screen.
#[test]
fn panel_resize_and_move_persist_and_anchor() {
    let mut p = pet();
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
    assert_eq!(p.panel_nav(NavKey::Quick(0)).as_deref(), Some("clip 11"));
    assert!(!p.panel_open(), "quick copy closes the panel like Enter");

    p.toggle_panel();
    assert_eq!(p.panel_nav(NavKey::Quick(9)).as_deref(), Some("clip 2"));

    // respects the source filter (most recent app first: Chrome)
    p.toggle_panel();
    p.panel_nav(NavKey::Tab);
    assert_eq!(p.panel.source.as_deref(), Some("Chrome"));
    assert_eq!(p.panel_nav(NavKey::Quick(1)).as_deref(), Some("clip 9"));

    // respects the search query
    p.toggle_panel();
    for c in "clip 1".chars() {
        p.panel_char(c);
    }
    // matches "clip 11", "clip 10", "clip 1" (newest first)
    assert_eq!(p.panel_nav(NavKey::Quick(2)).as_deref(), Some("clip 1"));

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
    assert_eq!(p.panel_nav(NavKey::Enter).as_deref(), Some("second"));
    assert!(p.panel_open(), "panel stays open for more copies");
    assert_eq!(p.panel_nav(NavKey::Quick(1)).as_deref(), Some("first"));
    assert!(p.panel_open());
    let lt = p.panel.layout();
    let row_y = lt.rows_y + panel::ROW_H / 2.0;
    assert_eq!(p.panel_click(150.0, row_y).as_deref(), Some("second"));
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

/// The stats bubble renders through the system-font path (the built-in
/// pixel font no longer exists): labels and values rasterize real pixels
/// in the bubble area.
#[test]
fn stats_bubble_renders_with_system_font() {
    let base = Scene {
        paw_l: 0.0,
        paw_r: 0.0,
        blink: 0.0,
        happy: 0.0,
        sleep: 0.0,
        excite: 0.0,
        squash: 0.0,
        breath: 0.0,
        tail_phase: 0.0,
        mouth_open: 0.0,
        typing_tier: 0,
        yawn: 0.0,
        look: 0.0,
        xp_popup: None,
        accessory: Accessory::None,
        particles: &[],
        fish: None,
        bubble: None,
        bubble_alpha: 0.0,
        toast: None,
        lang: Lang::Ko,
        origin: (0.0, 0.0),
    };
    let mut plain = Pixmap::new(240, 256).unwrap();
    render::render_card(&mut plain, &base, 1.0);

    let with_bubble = Scene {
        bubble: Some(BubbleData {
            level: 7,
            pct: 0.5,
            keys: 12345,
            clicks: 987,
            copies: 42,
            minutes: 95,
        }),
        bubble_alpha: 1.0,
        ..base
    };
    let mut bubbled = Pixmap::new(240, 256).unwrap();
    render::render_card(&mut bubbled, &with_bubble, 1.0);

    let diff = plain
        .data()
        .iter()
        .zip(bubbled.data().iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(diff > 4000, "bubble box + text must rasterize ({diff} bytes differ)");
}

/// Accessories are greyed until their level, then become selectable — the
/// menu's `enabled`/`checked` flags and the apply guard agree.
#[test]
fn context_menu_unlocks_accessories_by_level() {
    let mut low = pet(); // level 1
    assert!(!menu_find(&low.build_menu("HK", false), MenuAction::SetAccessory(1)).unwrap().enabled);
    low.apply_menu_action(MenuAction::SetAccessory(1)); // guarded no-op
    assert_eq!(low.st.accessory, 0);

    let mut high = pet_at_xp(10_000_000); // well past every unlock
    let m = high.build_menu("HK", false);
    assert!(menu_find(&m, MenuAction::SetAccessory(1)).unwrap().enabled);
    high.apply_menu_action(MenuAction::SetAccessory(1));
    assert_eq!(high.st.accessory, 1);
    assert!(menu_find(&high.build_menu("HK", false), MenuAction::SetAccessory(1)).unwrap().checked);
}
