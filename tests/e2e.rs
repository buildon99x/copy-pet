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
use clipcat::panel::{self, NavKey};
use clipcat::pet::Pet;
use clipcat::state::Persist;
use tiny_skia::Pixmap;

fn pet() -> Pet {
    let mut p = Pet::new(Persist::default());
    p.clips = ClipStore::default(); // keep tests off the real config dir
    p
}

fn copy(p: &mut Pet, text: &str, source: Option<&str>) {
    p.on_copy(text.to_string(), source.map(str::to_string), None);
}

/// The headline flow: copies arrive from several apps, the panel opens (as
/// the global hotkey does), Tab narrows to one app, Enter hands the picked
/// clip's text back for the OS clipboard, Esc unwinds filter then panel.
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

    p.panel_nav(NavKey::Tab);
    assert_eq!(p.panel.source.as_deref(), Some("Chrome"));
    let text = p.panel_nav(NavKey::Enter);
    assert_eq!(text.as_deref(), Some("breaking news headline"));

    // Esc: filter first, panel second
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

    // funnel header button engages the filter (pure panel state, no copy)
    let got = p.panel_click(panel::BTN_FILTER_X + 8.0, panel::BTN_Y + 8.0);
    assert_eq!(got, None);
    assert_eq!(p.panel.source.as_deref(), Some("Code"));

    // only Code clips remain; clicking the first row copies it back
    let row_y = panel::ROWS_Y + panel::ROW_H / 2.0;
    let text = p.panel_click(150.0, row_y);
    assert_eq!(text.as_deref(), Some("newest from code"));

    // the second row is empty under the filter -> a click there is a no-op
    let empty_y = panel::ROWS_Y + panel::ROW_H * 1.5;
    assert_eq!(p.panel_click(150.0, empty_y), None);

    // cycle past the last source: back to all apps, both rows clickable
    p.panel_click(panel::BTN_FILTER_X + 8.0, panel::BTN_Y + 8.0);
    assert_eq!(p.panel.source.as_deref(), Some("Chrome"));
    p.panel_click(panel::BTN_FILTER_X + 8.0, panel::BTN_Y + 8.0);
    assert_eq!(p.panel.source, None);
    assert_eq!(p.panel_click(150.0, empty_y).as_deref(), Some("older from chrome"));
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
    assert_eq!(
        (w, h),
        (panel::CANVAS_W as i32, panel::CANVAS_H as i32),
        "panel canvas at scale 1.0"
    );
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
