//! Clipboard panel: platform-agnostic UI state, layout geometry and hit
//! testing. The panel is drawn by [`crate::render::draw_panel`] above the cat
//! when open; both backends route mouse/keyboard events here and execute the
//! returned [`PanelAction`]s (only clipboard writes need the OS).
//!
//! All coordinates are in canvas units (window pixels / global scale).

use crate::clipboard::{Clip, ClipStore};
use std::cell::RefCell;

// ---- layout (canvas units) --------------------------------------------------

/// Full canvas size while the panel is open. The cat keeps its own
/// 240x256 canvas, drawn at [`CAT_ORIGIN`]; the panel card overlaps the
/// cat's (empty) bubble zone so the window doesn't get absurdly tall.
pub const CANVAS_W: f32 = 360.0;
pub const CANVAS_H: f32 = 542.0;
/// Top-left of the cat's 240x256 canvas inside the panel canvas.
pub const CAT_ORIGIN: (f32, f32) = (60.0, 286.0);

pub const CARD_X: f32 = 4.0;
pub const CARD_Y: f32 = 4.0;
pub const CARD_W: f32 = 352.0;
pub const CARD_H: f32 = 362.0;

/// Header buttons, 18x18, right-aligned: source filter, pause, clear,
/// language, close.
pub const BTN_Y: f32 = 10.0;
pub const BTN: f32 = 18.0;
pub const BTN_CLOSE_X: f32 = 330.0;
pub const BTN_LANG_X: f32 = 308.0;
pub const BTN_CLEAR_X: f32 = 286.0;
pub const BTN_PAUSE_X: f32 = 264.0;
pub const BTN_FILTER_X: f32 = 242.0;

pub const SEARCH_X: f32 = 12.0;
pub const SEARCH_Y: f32 = 34.0;
pub const SEARCH_W: f32 = 336.0;
pub const SEARCH_H: f32 = 20.0;

pub const ROWS_Y: f32 = 60.0;
pub const ROW_H: f32 = 34.0;
pub const VISIBLE_ROWS: usize = 8;
/// Row x-zones: pin toggle | clip body | delete.
pub const ROW_X: f32 = 12.0;
pub const ROW_W: f32 = 332.0;
pub const PIN_ZONE: f32 = 34.0; // x < ROW_X + PIN_ZONE
pub const DEL_ZONE: f32 = 28.0; // x > ROW_X + ROW_W - DEL_ZONE

pub const FOOTER_Y: f32 = 334.0;

/// What a panel interaction asks the app to do. Pure state changes
/// (scrolling, typing in search) are handled internally and return None.
#[derive(Debug, PartialEq)]
pub enum PanelAction {
    /// Put this clip's text back on the OS clipboard.
    Copy(u64),
    TogglePin(u64),
    Delete(u64),
    /// First press of the clear button: ask for the confirming second press.
    ArmClear,
    /// Clear all unpinned clips (the armed clear button pressed again).
    Clear,
    /// Restore the most recently deleted clip(s).
    Undo,
    /// Toggle clipboard capture on/off.
    ToggleCapture,
    ToggleLang,
    Close,
}

/// Keyboard navigation keys forwarded by the backends while the panel is open.
#[derive(Clone, Copy, PartialEq)]
pub enum NavKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Delete,
    Backspace,
    Esc,
    /// Cycles the source-app filter (all -> app 1 -> app 2 -> ... -> all).
    Tab,
    /// Toggle the pin on the selected clip (Ctrl+P).
    Pin,
    /// Restore the most recently deleted clip(s) (Ctrl+Z).
    Undo,
}

#[derive(Default)]
pub struct Panel {
    pub open: bool,
    pub query: String,
    /// Active source-app filter; None shows clips from every app.
    pub source: Option<String>,
    /// Index of the first visible row into the filtered clip list.
    pub scroll: usize,
    /// Keyboard selection index into the filtered clip list.
    pub sel: usize,
    /// Cursor position in canvas coords, for hover highlight.
    pub cursor: Option<(f32, f32)>,
    /// The clear button was pressed once; the next press really clears.
    /// Any other interaction disarms it.
    pub clear_armed: bool,
    /// Cached filtered row order (the panel is redrawn every tick while
    /// open; without this the query would re-scan every clip text per frame).
    cache: RefCell<ViewCache>,
}

#[derive(Default)]
struct ViewCache {
    valid: bool,
    version: u64,
    query: String,
    source: Option<String>,
    rows: Vec<usize>,
}

impl Panel {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.query.clear();
            self.source = None;
            self.scroll = 0;
            self.sel = 0;
        }
        self.clear_armed = false;
    }

    /// Re-clamps scroll/selection after the store changed under the panel
    /// (delete, clear, undo).
    pub fn refresh(&mut self, store: &ClipStore) {
        let total = self.visible(store).len();
        self.clamp_scroll(total);
        self.keep_sel_visible();
    }

    /// Moves the keyboard selection to the clip with `id` (after a pin
    /// toggle or undo re-ordered the list) and keeps it on screen.
    pub fn focus_id(&mut self, store: &ClipStore, id: u64) {
        let visible = self.visible(store);
        if let Some(i) = visible.iter().position(|c| c.id == id) {
            self.sel = i;
        }
        let total = visible.len();
        self.keep_sel_visible();
        self.clamp_scroll(total);
    }

    /// The clip list the panel currently shows (query + source filter).
    /// Recomputed only when the query, the filter or the store changed.
    pub fn visible<'a>(&self, store: &'a ClipStore) -> Vec<&'a Clip> {
        let mut c = self.cache.borrow_mut();
        if !c.valid
            || c.version != store.version()
            || c.query != self.query
            || c.source != self.source
        {
            c.rows = store.filtered_indices(&self.query, self.source.as_deref());
            c.version = store.version();
            c.query.clone_from(&self.query);
            c.source.clone_from(&self.source);
            c.valid = true;
        }
        c.rows.iter().filter_map(|&i| store.by_index(i)).collect()
    }

    /// Advances the source filter to the next app seen in the history,
    /// wrapping back to "all apps" after the last one.
    pub fn cycle_source(&mut self, store: &ClipStore) {
        let sources = store.sources();
        let next = match &self.source {
            None => sources.first().map(|s| s.to_string()),
            Some(cur) => sources
                .iter()
                .position(|s| s.eq_ignore_ascii_case(cur))
                .and_then(|i| sources.get(i + 1))
                .map(|s| s.to_string()),
        };
        self.source = next;
        self.scroll = 0;
        self.sel = 0;
    }

    /// True if the point (canvas coords) is inside the panel card.
    pub fn hit(&self, x: f32, y: f32) -> bool {
        self.open
            && (CARD_X..=CARD_X + CARD_W).contains(&x)
            && (CARD_Y..=CARD_Y + CARD_H).contains(&y)
    }

    fn clamp_scroll(&mut self, total: usize) {
        let max = total.saturating_sub(VISIBLE_ROWS);
        self.scroll = self.scroll.min(max);
        self.sel = self.sel.min(total.saturating_sub(1));
    }

    fn keep_sel_visible(&mut self) {
        if self.sel < self.scroll {
            self.scroll = self.sel;
        } else if self.sel >= self.scroll + VISIBLE_ROWS {
            self.scroll = self.sel + 1 - VISIBLE_ROWS;
        }
    }

    /// Row index (into the filtered list) under the y coordinate, if any.
    pub fn row_at(&self, y: f32, total: usize) -> Option<usize> {
        if y < ROWS_Y || y >= ROWS_Y + ROW_H * VISIBLE_ROWS as f32 {
            return None;
        }
        let i = self.scroll + ((y - ROWS_Y) / ROW_H) as usize;
        (i < total).then_some(i)
    }

    /// Mouse wheel over the panel: scroll by rows (positive = down).
    pub fn wheel(&mut self, rows: i32, store: &ClipStore) {
        let total = self.visible(store).len();
        if rows > 0 {
            self.scroll = self.scroll.saturating_add(rows as usize);
        } else {
            self.scroll = self.scroll.saturating_sub((-rows) as usize);
        }
        self.clamp_scroll(total);
    }

    /// A left click at canvas coords. Returns the action to perform.
    pub fn click(&mut self, x: f32, y: f32, store: &ClipStore) -> Option<PanelAction> {
        if !self.hit(x, y) {
            return None;
        }
        // header buttons
        let on_btn = |bx: f32| {
            x >= bx - 2.0 && x <= bx + BTN + 2.0 && (BTN_Y - 2.0..=BTN_Y + BTN + 2.0).contains(&y)
        };
        if on_btn(BTN_CLEAR_X) {
            // two presses to clear everything: arm first, clear on the second
            self.clear_armed = !self.clear_armed;
            return Some(if self.clear_armed {
                PanelAction::ArmClear
            } else {
                PanelAction::Clear
            });
        }
        self.clear_armed = false;
        if on_btn(BTN_CLOSE_X) {
            return Some(PanelAction::Close);
        }
        if on_btn(BTN_LANG_X) {
            return Some(PanelAction::ToggleLang);
        }
        if on_btn(BTN_PAUSE_X) {
            return Some(PanelAction::ToggleCapture);
        }
        if on_btn(BTN_FILTER_X) {
            // pure panel state: cycle the source filter, no action needed
            self.cycle_source(store);
            return None;
        }
        // clip rows
        let visible = self.visible(store);
        if let Some(i) = self.row_at(y, visible.len()) {
            let id = visible[i].id;
            self.sel = i;
            if x < ROW_X + PIN_ZONE {
                return Some(PanelAction::TogglePin(id));
            }
            if x > ROW_X + ROW_W - DEL_ZONE {
                return Some(PanelAction::Delete(id));
            }
            return Some(PanelAction::Copy(id));
        }
        None
    }

    /// Printable character typed while the panel is open: search input.
    pub fn input_char(&mut self, c: char) {
        self.clear_armed = false;
        if c.is_control() {
            return;
        }
        if self.query.chars().count() < 60 {
            self.query.push(c);
            self.scroll = 0;
            self.sel = 0;
        }
    }

    /// Navigation key while the panel is open.
    pub fn nav(&mut self, key: NavKey, store: &ClipStore) -> Option<PanelAction> {
        let armed = std::mem::take(&mut self.clear_armed);
        let total = self.visible(store).len();
        match key {
            NavKey::Up => {
                self.sel = self.sel.saturating_sub(1);
                self.keep_sel_visible();
            }
            NavKey::Down => {
                if self.sel + 1 < total {
                    self.sel += 1;
                }
                self.keep_sel_visible();
            }
            NavKey::PageUp => {
                self.sel = self.sel.saturating_sub(VISIBLE_ROWS);
                self.keep_sel_visible();
            }
            NavKey::PageDown => {
                self.sel = (self.sel + VISIBLE_ROWS).min(total.saturating_sub(1));
                self.keep_sel_visible();
            }
            NavKey::Home => {
                self.sel = 0;
                self.keep_sel_visible();
            }
            NavKey::End => {
                self.sel = total.saturating_sub(1);
                self.keep_sel_visible();
            }
            NavKey::Enter => {
                let visible = self.visible(store);
                if let Some(c) = visible.get(self.sel) {
                    return Some(PanelAction::Copy(c.id));
                }
            }
            NavKey::Delete => {
                let visible = self.visible(store);
                if let Some(c) = visible.get(self.sel) {
                    return Some(PanelAction::Delete(c.id));
                }
            }
            NavKey::Pin => {
                let visible = self.visible(store);
                if let Some(c) = visible.get(self.sel) {
                    return Some(PanelAction::TogglePin(c.id));
                }
            }
            NavKey::Undo => return Some(PanelAction::Undo),
            NavKey::Backspace => {
                self.query.pop();
                self.scroll = 0;
                self.sel = 0;
            }
            NavKey::Tab => {
                self.cycle_source(store);
                return None;
            }
            NavKey::Esc => {
                // peel back one layer at a time: armed clear, query, filter, close
                if armed {
                    return None;
                }
                if !self.query.is_empty() {
                    self.query.clear();
                } else if self.source.is_some() {
                    self.source = None;
                } else {
                    return Some(PanelAction::Close);
                }
                self.scroll = 0;
                self.sel = 0;
            }
        }
        self.clamp_scroll(total);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::ClipStore;

    fn store(n: usize) -> ClipStore {
        let mut s = ClipStore::default();
        for i in 0..n {
            s.add_copy(format!("clip number {i}"), None);
        }
        s
    }

    #[test]
    fn click_routes_rows_and_zones() {
        let s = store(3);
        let mut p = Panel { open: true, ..Default::default() };
        let ids: Vec<u64> = s.visible("").iter().map(|c| c.id).collect();
        // middle of first row copies
        let y0 = ROWS_Y + ROW_H / 2.0;
        assert_eq!(p.click(150.0, y0, &s), Some(PanelAction::Copy(ids[0])));
        // pin zone
        assert_eq!(p.click(ROW_X + 5.0, y0, &s), Some(PanelAction::TogglePin(ids[0])));
        // delete zone
        assert_eq!(
            p.click(ROW_X + ROW_W - 5.0, y0, &s),
            Some(PanelAction::Delete(ids[0]))
        );
        // outside the card
        assert_eq!(p.click(150.0, 400.0, &s), None);
        // close button
        assert_eq!(p.click(BTN_CLOSE_X + 8.0, BTN_Y + 8.0, &s), Some(PanelAction::Close));
    }

    #[test]
    fn empty_row_area_clicks_do_nothing() {
        let s = store(1);
        let mut p = Panel { open: true, ..Default::default() };
        let y = ROWS_Y + ROW_H * 3.5; // row 3, but only 1 clip
        assert_eq!(p.click(150.0, y, &s), None);
    }

    #[test]
    fn scroll_clamps() {
        let s = store(10);
        let mut p = Panel { open: true, ..Default::default() };
        p.wheel(100, &s);
        assert_eq!(p.scroll, 10 - VISIBLE_ROWS);
        p.wheel(-100, &s);
        assert_eq!(p.scroll, 0);
    }

    #[test]
    fn keyboard_selection_follows_scroll() {
        let s = store(10);
        let mut p = Panel { open: true, ..Default::default() };
        for _ in 0..8 {
            p.nav(NavKey::Down, &s);
        }
        assert_eq!(p.sel, 8);
        assert!(p.sel >= p.scroll && p.sel < p.scroll + VISIBLE_ROWS);
        let act = p.nav(NavKey::Enter, &s);
        let ids: Vec<u64> = s.visible("").iter().map(|c| c.id).collect();
        assert_eq!(act, Some(PanelAction::Copy(ids[8])));
    }

    #[test]
    fn esc_clears_query_then_filter_then_closes() {
        let mut s = store(2);
        s.add_copy("from chrome".into(), Some("Chrome".into()));
        let mut p = Panel { open: true, ..Default::default() };
        p.input_char('a');
        p.cycle_source(&s);
        assert!(p.source.is_some());
        assert_eq!(p.nav(NavKey::Esc, &s), None);
        assert!(p.query.is_empty());
        assert!(p.source.is_some(), "query clears first");
        assert_eq!(p.nav(NavKey::Esc, &s), None);
        assert!(p.source.is_none(), "filter clears second");
        assert_eq!(p.nav(NavKey::Esc, &s), Some(PanelAction::Close));
    }

    fn store_with_sources() -> ClipStore {
        let mut s = ClipStore::default();
        s.add_copy("alpha".into(), Some("Chrome".into()));
        s.add_copy("beta".into(), Some("Code".into()));
        s.add_copy("gamma".into(), None);
        s.add_copy("delta".into(), Some("Chrome".into()));
        s
    }

    #[test]
    fn source_filter_cycles_and_filters_rows() {
        let s = store_with_sources();
        let mut p = Panel { open: true, ..Default::default() };
        assert_eq!(p.visible(&s).len(), 4);
        // most recent source first: Chrome, then Code, then back to all
        p.cycle_source(&s);
        assert_eq!(p.source.as_deref(), Some("Chrome"));
        assert_eq!(p.visible(&s).len(), 2);
        p.cycle_source(&s);
        assert_eq!(p.source.as_deref(), Some("Code"));
        assert_eq!(p.visible(&s).len(), 1);
        p.cycle_source(&s);
        assert_eq!(p.source, None);
        assert_eq!(p.visible(&s).len(), 4);
    }

    #[test]
    fn filter_button_and_tab_cycle_the_source() {
        let s = store_with_sources();
        let mut p = Panel { open: true, ..Default::default() };
        assert_eq!(p.click(BTN_FILTER_X + 8.0, BTN_Y + 8.0, &s), None);
        assert_eq!(p.source.as_deref(), Some("Chrome"));
        assert_eq!(p.nav(NavKey::Tab, &s), None);
        assert_eq!(p.source.as_deref(), Some("Code"));
        // enter copies from the *filtered* list
        let act = p.nav(NavKey::Enter, &s);
        let code_id = s.visible_filtered("", Some("Code"))[0].id;
        assert_eq!(act, Some(PanelAction::Copy(code_id)));
        // reopening the panel clears the filter
        p.toggle();
        p.toggle();
        assert_eq!(p.source, None);
    }

    #[test]
    fn clear_needs_a_second_press() {
        let s = store(3);
        let mut p = Panel { open: true, ..Default::default() };
        let (bx, by) = (BTN_CLEAR_X + 8.0, BTN_Y + 8.0);
        assert_eq!(p.click(bx, by, &s), Some(PanelAction::ArmClear));
        assert!(p.clear_armed);
        assert_eq!(p.click(bx, by, &s), Some(PanelAction::Clear));
        assert!(!p.clear_armed);
        // any other interaction disarms instead of clearing
        assert_eq!(p.click(bx, by, &s), Some(PanelAction::ArmClear));
        p.input_char('x');
        assert!(!p.clear_armed);
        assert_eq!(p.click(bx, by, &s), Some(PanelAction::ArmClear));
        assert_eq!(p.nav(NavKey::Esc, &s), None, "esc disarms, doesn't close");
        assert!(!p.clear_armed);
        assert!(p.open);
    }

    #[test]
    fn home_end_jump_selection() {
        let s = store(20);
        let mut p = Panel { open: true, ..Default::default() };
        p.nav(NavKey::End, &s);
        assert_eq!(p.sel, 19);
        assert!(p.sel >= p.scroll && p.sel < p.scroll + VISIBLE_ROWS);
        p.nav(NavKey::Home, &s);
        assert_eq!((p.sel, p.scroll), (0, 0));
    }

    #[test]
    fn pin_and_undo_keys_act_on_selection() {
        let s = store(3);
        let mut p = Panel { open: true, ..Default::default() };
        p.nav(NavKey::Down, &s);
        let id = s.visible("")[1].id;
        assert_eq!(p.nav(NavKey::Pin, &s), Some(PanelAction::TogglePin(id)));
        assert_eq!(p.nav(NavKey::Undo, &s), Some(PanelAction::Undo));
    }

    #[test]
    fn visible_cache_tracks_store_query_and_filter() {
        let mut s = store(3);
        let p = Panel { open: true, ..Default::default() };
        assert_eq!(p.visible(&s).len(), 3);
        // store mutations invalidate the cached view immediately
        s.add_copy("clip number 99".into(), None);
        assert_eq!(p.visible(&s).len(), 4);
        let id = p.visible(&s)[0].id;
        s.delete(id);
        assert_eq!(p.visible(&s).len(), 3);
        s.undo_delete();
        assert_eq!(p.visible(&s).len(), 4);
        // query changes invalidate it too
        let mut p = p;
        p.input_char('9');
        assert_eq!(p.visible(&s).len(), 1);
        p.nav(NavKey::Backspace, &s);
        assert_eq!(p.visible(&s).len(), 4);
    }

    #[test]
    fn typing_filters_reset_scroll() {
        let s = store(20);
        let mut p = Panel { open: true, ..Default::default() };
        p.wheel(5, &s);
        assert!(p.scroll > 0);
        p.input_char('7');
        assert_eq!(p.scroll, 0);
        assert_eq!(s.visible(&p.query).len(), 2); // "clip number 7", "...17"
    }
}
