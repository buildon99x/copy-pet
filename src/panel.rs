//! Clipboard panel: platform-agnostic UI state, layout geometry and hit
//! testing. The panel is drawn by [`crate::render::draw_panel`] above the cat
//! when open; both backends route mouse/keyboard events here and execute the
//! returned [`PanelAction`]s (only clipboard writes need the OS).
//!
//! All coordinates are in canvas units (window pixels / global scale).

use crate::clipboard::ClipStore;

// ---- layout (canvas units) --------------------------------------------------

/// Full canvas size while the panel is open. The cat keeps its own
/// 240x256 canvas, drawn at [`CAT_ORIGIN`]; the panel card overlaps the
/// cat's (empty) bubble zone so the window doesn't get absurdly tall.
pub const CANVAS_W: f32 = 324.0;
pub const CANVAS_H: f32 = 426.0;
/// Top-left of the cat's 240x256 canvas inside the panel canvas.
pub const CAT_ORIGIN: (f32, f32) = (42.0, 170.0);

pub const CARD_X: f32 = 4.0;
pub const CARD_Y: f32 = 4.0;
pub const CARD_W: f32 = 316.0;
pub const CARD_H: f32 = 246.0;

/// Header buttons, 16x16, right-aligned: pause, clear, language, close.
pub const BTN_Y: f32 = 9.0;
pub const BTN: f32 = 16.0;
pub const BTN_CLOSE_X: f32 = 300.0;
pub const BTN_LANG_X: f32 = 280.0;
pub const BTN_CLEAR_X: f32 = 260.0;
pub const BTN_PAUSE_X: f32 = 240.0;

pub const SEARCH_X: f32 = 12.0;
pub const SEARCH_Y: f32 = 30.0;
pub const SEARCH_W: f32 = 300.0;
pub const SEARCH_H: f32 = 18.0;

pub const ROWS_Y: f32 = 54.0;
pub const ROW_H: f32 = 28.0;
pub const VISIBLE_ROWS: usize = 6;
/// Row x-zones: pin toggle | clip body | delete.
pub const ROW_X: f32 = 12.0;
pub const ROW_W: f32 = 296.0;
pub const PIN_ZONE: f32 = 32.0; // x < ROW_X + PIN_ZONE
pub const DEL_ZONE: f32 = 24.0; // x > ROW_X + ROW_W - DEL_ZONE

pub const FOOTER_Y: f32 = 228.0;

/// What a panel interaction asks the app to do. Pure state changes
/// (scrolling, typing in search) are handled internally and return None.
#[derive(Debug, PartialEq)]
pub enum PanelAction {
    /// Put this clip's text back on the OS clipboard.
    Copy(u64),
    TogglePin(u64),
    Delete(u64),
    /// Clear all unpinned clips.
    Clear,
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
    Enter,
    Delete,
    Backspace,
    Esc,
}

#[derive(Default)]
pub struct Panel {
    pub open: bool,
    pub query: String,
    /// Index of the first visible row into the filtered clip list.
    pub scroll: usize,
    /// Keyboard selection index into the filtered clip list.
    pub sel: usize,
    /// Cursor position in canvas coords, for hover highlight.
    pub cursor: Option<(f32, f32)>,
}

impl Panel {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.query.clear();
            self.scroll = 0;
            self.sel = 0;
        }
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
        let total = store.visible(&self.query).len();
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
        if on_btn(BTN_CLOSE_X) {
            return Some(PanelAction::Close);
        }
        if on_btn(BTN_LANG_X) {
            return Some(PanelAction::ToggleLang);
        }
        if on_btn(BTN_CLEAR_X) {
            return Some(PanelAction::Clear);
        }
        if on_btn(BTN_PAUSE_X) {
            return Some(PanelAction::ToggleCapture);
        }
        // clip rows
        let visible = store.visible(&self.query);
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
        let total = store.visible(&self.query).len();
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
            NavKey::Enter => {
                let visible = store.visible(&self.query);
                if let Some(c) = visible.get(self.sel) {
                    return Some(PanelAction::Copy(c.id));
                }
            }
            NavKey::Delete => {
                let visible = store.visible(&self.query);
                if let Some(c) = visible.get(self.sel) {
                    return Some(PanelAction::Delete(c.id));
                }
            }
            NavKey::Backspace => {
                self.query.pop();
                self.scroll = 0;
                self.sel = 0;
            }
            NavKey::Esc => {
                if self.query.is_empty() {
                    return Some(PanelAction::Close);
                }
                self.query.clear();
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
    fn esc_clears_query_then_closes() {
        let s = store(2);
        let mut p = Panel { open: true, ..Default::default() };
        p.input_char('a');
        assert_eq!(p.nav(NavKey::Esc, &s), None);
        assert!(p.query.is_empty());
        assert_eq!(p.nav(NavKey::Esc, &s), Some(PanelAction::Close));
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
