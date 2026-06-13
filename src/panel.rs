//! Clipboard panel: platform-agnostic UI state, layout geometry and hit
//! testing. The panel is drawn by [`crate::render::draw_panel`] above the cat
//! when open; both backends route mouse/keyboard events here and execute the
//! returned [`PanelAction`]s (only clipboard writes need the OS).
//!
//! The card is movable and resizable: its size and its offset relative to
//! the cat are user state (persisted in `state.json`), and [`Panel::layout`]
//! derives the whole geometry — including the window canvas, which is the
//! union of the cat's canvas and the card — from them. Dragging the header
//! strip moves the card, dragging the bottom-right grip resizes it (see
//! [`Panel::drag_hit`] / [`Panel::drag_by`]).
//!
//! All coordinates are in canvas units (window pixels / global scale).

use crate::clipboard::{Clip, ClipStore};
use std::cell::RefCell;

// ---- layout (canvas units) --------------------------------------------------

/// Card size limits. The defaults reproduce the original fixed panel.
pub const MIN_W: f32 = 280.0;
pub const MAX_W: f32 = 600.0;
pub const MIN_H: f32 = 220.0;
pub const MAX_H: f32 = 700.0;
pub const DEFAULT_W: f32 = 352.0;
pub const DEFAULT_H: f32 = 362.0;
/// Default card offset relative to the cat's top-left: centered above it.
pub const DEFAULT_OFF: (f32, f32) = (-56.0, -282.0);
/// How far the card may wander from the cat. Generous on purpose — the
/// card can sit anywhere on a desktop-sized area (the union canvas grows
/// with it); the clamp only exists so hand-edited state or a runaway drag
/// can't produce an absurd canvas.
pub const MAX_OFF: f32 = 4096.0;

/// Expanded "full screen" mode: a fixed-size three-pane card (sidebar, clip
/// list, clip detail). Not user-resizable — the compact card's w/h/off are
/// kept separately and restored on collapse. See ADR-0012.
pub const EXP_W: f32 = 760.0;
pub const EXP_H: f32 = 560.0;
/// Expanded card offset relative to the cat: centers the cat below the card.
pub const EXP_OFF: (f32, f32) = (-260.0, -564.0);

pub const BTN: f32 = 18.0;
pub const ROW_H: f32 = 34.0;
/// Row x-zones: pin toggle | clip body | delete.
pub const PIN_ZONE: f32 = 34.0; // x < row_x + PIN_ZONE
pub const DEL_ZONE: f32 = 28.0; // x > row_x + row_w - DEL_ZONE
/// Quick-copy hotkeys cover the first `QUICK_KEYS` rows (Ctrl+0..9).
pub const QUICK_KEYS: usize = 10;
/// Side of the square resize grip in the card's bottom-right corner.
pub const GRIP: f32 = 18.0;

/// Card-top to first clip row (title/buttons + search box).
const HEADER_H: f32 = 56.0;
/// Last clip row to card bottom (count + shortcut help lines).
const FOOTER_H: f32 = 32.0;
/// Canvas margin around the card.
const MARGIN: f32 = 4.0;

/// Clamps persisted card geometry to sane bounds: hand-edited or legacy
/// `state.json` values (including NaN/inf) must never produce an absurd
/// layout. Returns (w, h, off_x, off_y).
pub fn clamp_geometry(w: f32, h: f32, off_x: f32, off_y: f32) -> (f32, f32, f32, f32) {
    let or = |v: f32, d: f32| if v.is_finite() { v } else { d };
    (
        or(w, DEFAULT_W).clamp(MIN_W, MAX_W),
        or(h, DEFAULT_H).clamp(MIN_H, MAX_H),
        or(off_x, DEFAULT_OFF.0).clamp(-MAX_OFF, MAX_OFF),
        or(off_y, DEFAULT_OFF.1).clamp(-MAX_OFF, MAX_OFF),
    )
}

/// Computed panel geometry for the current card size/offset, all in canvas
/// units. The canvas is the union of the cat's 240x256 canvas and the card
/// (plus a margin); `cat` is where the cat canvas sits inside it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    pub canvas_w: f32,
    pub canvas_h: f32,
    /// Top-left of the cat canvas inside the window canvas.
    pub cat: (f32, f32),
    pub card_x: f32,
    pub card_y: f32,
    pub card_w: f32,
    pub card_h: f32,
    /// Header buttons, right-aligned: filter, pause, clear, language, close.
    pub btn_y: f32,
    pub btn_close_x: f32,
    pub btn_lang_x: f32,
    pub btn_clear_x: f32,
    pub btn_pause_x: f32,
    pub btn_filter_x: f32,
    pub search_x: f32,
    pub search_y: f32,
    pub search_w: f32,
    pub search_h: f32,
    pub rows_y: f32,
    pub row_x: f32,
    pub row_w: f32,
    /// Clip rows that fit between the header and the footer.
    pub rows: usize,
    pub footer_y: f32,
}

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
    /// Quick copy (Ctrl+0..9): copies the nth visible clip from the top
    /// (0 = the topmost row, matching the digit badges on the rows).
    Quick(u8),
}

/// A drag started on the card: the header strip moves it, the grip resizes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PanelDrag {
    Move,
    Resize,
}

/// What a click in the expanded screen landed on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ExpandedHit {
    None,
    Collapse,
    Nav(NavView),
    Row(usize),
    Action(ExpAction),
}

/// A detail-pane action button in the expanded screen.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ExpAction {
    Copy,
    Pin,
    Delete,
}

/// Sidebar destinations in the expanded screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavView {
    Clipboard,
    Pinned,
    Statistics,
    Customization,
    Settings,
}

impl NavView {
    /// Nav items, top to bottom.
    pub const ALL: [NavView; 5] = [
        NavView::Clipboard,
        NavView::Pinned,
        NavView::Statistics,
        NavView::Customization,
        NavView::Settings,
    ];
}

/// Geometry of the expanded three-pane screen (canvas units), derived from the
/// card rect in [`Layout`]. The sidebar lists [`NavView`]s, the center holds
/// the search + clip rows, the right pane shows the selected clip's detail.
#[derive(Clone, Copy, Debug)]
pub struct ExpandedLayout {
    pub card: (f32, f32, f32, f32),
    pub sidebar: (f32, f32, f32, f32),
    pub list: (f32, f32, f32, f32),
    pub detail: (f32, f32, f32, f32),
    /// Collapse-to-compact button (top-right of the card).
    pub collapse: (f32, f32, f32, f32),
    /// First nav item's top y and the per-item height (sidebar).
    pub nav_y0: f32,
    pub nav_h: f32,
    /// Search box in the list column.
    pub search: (f32, f32, f32, f32),
    pub rows_y: f32,
    pub rows: usize,
    /// Detail action buttons start y + per-button height.
    pub action_y0: f32,
    pub action_h: f32,
}

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
    /// Card size in canvas units (user-resizable, persisted).
    pub w: f32,
    pub h: f32,
    /// Card top-left relative to the cat's top-left (user-movable, persisted).
    pub off: (f32, f32),
    /// Expanded three-pane "full screen" mode (vs. the compact panel).
    pub expanded: bool,
    /// Selected sidebar destination while expanded.
    pub nav: NavView,
    /// Cached filtered row order (the panel is redrawn every tick while
    /// open; without this the query would re-scan every clip text per frame).
    cache: RefCell<ViewCache>,
}

impl Default for Panel {
    fn default() -> Panel {
        Panel {
            open: false,
            query: String::new(),
            source: None,
            scroll: 0,
            sel: 0,
            cursor: None,
            clear_armed: false,
            w: DEFAULT_W,
            h: DEFAULT_H,
            off: DEFAULT_OFF,
            expanded: false,
            nav: NavView::Clipboard,
            cache: RefCell::new(ViewCache::default()),
        }
    }
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
    /// Panel with persisted card geometry (clamped to sane bounds).
    pub fn with_geometry(w: f32, h: f32, off: (f32, f32)) -> Panel {
        let (w, h, ox, oy) = clamp_geometry(w, h, off.0, off.1);
        Panel {
            w,
            h,
            off: (ox, oy),
            ..Default::default()
        }
    }

    /// The full geometry for the current card size/offset. In expanded mode
    /// the card is the fixed three-pane size/offset instead of the user's.
    pub fn layout(&self) -> Layout {
        let (w, h, off_x, off_y) = if self.expanded {
            (EXP_W, EXP_H, EXP_OFF.0, EXP_OFF.1)
        } else {
            (self.w, self.h, self.off.0, self.off.1)
        };
        // canvas = union of the cat canvas and the margin-padded card
        let left = (off_x - MARGIN).min(0.0);
        let top = (off_y - MARGIN).min(0.0);
        let right = (off_x + w + MARGIN).max(crate::render::CANVAS_W);
        let bottom = (off_y + h + MARGIN).max(crate::render::CANVAS_H);
        let cat = (-left, -top);
        let card_x = cat.0 + off_x;
        let card_y = cat.1 + off_y;
        let btn_close_x = card_x + w - 26.0;
        Layout {
            canvas_w: right - left,
            canvas_h: bottom - top,
            cat,
            card_x,
            card_y,
            card_w: w,
            card_h: h,
            btn_y: card_y + 6.0,
            btn_close_x,
            btn_lang_x: btn_close_x - 22.0,
            btn_clear_x: btn_close_x - 44.0,
            btn_pause_x: btn_close_x - 66.0,
            btn_filter_x: btn_close_x - 88.0,
            search_x: card_x + 8.0,
            search_y: card_y + 30.0,
            search_w: w - 16.0,
            search_h: 20.0,
            rows_y: card_y + HEADER_H,
            row_x: card_x + 8.0,
            row_w: w - 20.0,
            rows: (((h - HEADER_H - FOOTER_H) / ROW_H) as usize).max(1),
            footer_y: card_y + h - FOOTER_H,
        }
    }

    /// Clip rows that fit on screen with the current card height.
    pub fn visible_rows(&self) -> usize {
        if self.expanded {
            self.expanded_layout().rows
        } else {
            self.layout().rows
        }
    }

    /// Sub-rects of the expanded three-pane screen (only meaningful while
    /// `expanded`); derived from the card rect in [`Panel::layout`].
    pub fn expanded_layout(&self) -> ExpandedLayout {
        let l = self.layout();
        let (cx, cy, cw, ch) = (l.card_x, l.card_y, l.card_w, l.card_h);
        let sidebar_w = 196.0;
        let detail_w = 244.0;
        let list_w = cw - sidebar_w - detail_w;
        let list = (cx + sidebar_w, cy, list_w, ch);
        ExpandedLayout {
            card: (cx, cy, cw, ch),
            sidebar: (cx, cy, sidebar_w, ch),
            list,
            detail: (cx + sidebar_w + list_w, cy, detail_w, ch),
            collapse: (cx + cw - 26.0, cy + 8.0, 18.0, 18.0),
            nav_y0: cy + 176.0,
            nav_h: 30.0,
            search: (list.0 + 12.0, cy + 44.0, list_w - 24.0, 22.0),
            rows_y: cy + 80.0,
            rows: (((ch - 80.0 - 34.0) / ROW_H) as usize).max(1),
            action_y0: cy + 246.0,
            action_h: 26.0,
        }
    }

    /// Enter/leave the expanded three-pane screen.
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
        self.clear_armed = false;
        self.scroll = 0;
        if self.expanded {
            self.nav = NavView::Clipboard;
        }
    }

    /// Clip rows the expanded list shows for the current nav: the filtered
    /// history for Clipboard, only pinned clips for Pinned.
    pub fn expanded_visible<'a>(&self, store: &'a ClipStore) -> Vec<&'a Clip> {
        let all = self.visible(store);
        match self.nav {
            NavView::Pinned => all.into_iter().filter(|c| c.pinned).collect(),
            _ => all,
        }
    }

    /// What a click at canvas coords landed on in the expanded screen.
    pub fn expanded_hit(&self, x: f32, y: f32, store: &ClipStore) -> ExpandedHit {
        let el = self.expanded_layout();
        let inr = |r: (f32, f32, f32, f32)| x >= r.0 && x <= r.0 + r.2 && y >= r.1 && y <= r.1 + r.3;
        if inr(el.collapse) {
            return ExpandedHit::Collapse;
        }
        let (sx, _, sw, _) = el.sidebar;
        for (i, nav) in NavView::ALL.iter().enumerate() {
            let ny = el.nav_y0 + i as f32 * el.nav_h;
            if x >= sx + 8.0 && x <= sx + sw - 8.0 && y >= ny && y < ny + el.nav_h - 4.0 {
                return ExpandedHit::Nav(*nav);
            }
        }
        let has_sel = self.sel < self.expanded_visible(store).len();
        let (dx, _, dw, _) = el.detail;
        let (px, pw) = (dx + 14.0, dw - 28.0);
        for (i, act) in [ExpAction::Copy, ExpAction::Pin, ExpAction::Delete].into_iter().enumerate() {
            let by = el.action_y0 + i as f32 * (el.action_h + 6.0);
            if has_sel && x >= px && x <= px + pw && y >= by && y <= by + el.action_h {
                return ExpandedHit::Action(act);
            }
        }
        let (lxc, _, lwc, _) = el.list;
        if x >= lxc && x <= lxc + lwc && y >= el.rows_y {
            let i = ((y - el.rows_y) / ROW_H) as usize;
            if i < el.rows {
                let idx = self.scroll + i;
                if idx < self.expanded_visible(store).len() {
                    return ExpandedHit::Row(idx);
                }
            }
        }
        ExpandedHit::None
    }

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

    /// Zones that start a card drag: the bottom-right grip resizes, the
    /// header strip left of the buttons moves the card.
    pub fn drag_hit(&self, x: f32, y: f32) -> Option<PanelDrag> {
        if !self.open {
            return None;
        }
        let l = self.layout();
        let (x1, y1) = (l.card_x + l.card_w, l.card_y + l.card_h);
        if x >= x1 - GRIP && x <= x1 + 2.0 && y >= y1 - GRIP && y <= y1 + 2.0 {
            return Some(PanelDrag::Resize);
        }
        if self.hit(x, y) && y < l.search_y - 2.0 && x < l.btn_filter_x - 4.0 {
            return Some(PanelDrag::Move);
        }
        None
    }

    /// Applies a drag delta in canvas units, clamping to the size/offset
    /// bounds. The caller re-clamps scroll afterwards ([`Panel::refresh`]).
    pub fn drag_by(&mut self, kind: PanelDrag, dx: f32, dy: f32) {
        match kind {
            PanelDrag::Move => {
                self.off.0 = (self.off.0 + dx).clamp(-MAX_OFF, MAX_OFF);
                self.off.1 = (self.off.1 + dy).clamp(-MAX_OFF, MAX_OFF);
            }
            PanelDrag::Resize => {
                self.w = (self.w + dx).clamp(MIN_W, MAX_W);
                self.h = (self.h + dy).clamp(MIN_H, MAX_H);
            }
        }
    }

    /// Re-clamps scroll/selection after the store changed under the panel
    /// (delete, clear, undo) or the card was resized.
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
        if !self.open {
            return false;
        }
        let l = self.layout();
        (l.card_x..=l.card_x + l.card_w).contains(&x)
            && (l.card_y..=l.card_y + l.card_h).contains(&y)
    }

    fn clamp_scroll(&mut self, total: usize) {
        let max = total.saturating_sub(self.visible_rows());
        self.scroll = self.scroll.min(max);
        self.sel = self.sel.min(total.saturating_sub(1));
    }

    fn keep_sel_visible(&mut self) {
        let rows = self.visible_rows();
        if self.sel < self.scroll {
            self.scroll = self.sel;
        } else if self.sel >= self.scroll + rows {
            self.scroll = self.sel + 1 - rows;
        }
    }

    /// Row index (into the filtered list) under the y coordinate, if any.
    pub fn row_at(&self, y: f32, total: usize) -> Option<usize> {
        let l = self.layout();
        if y < l.rows_y || y >= l.rows_y + ROW_H * l.rows as f32 {
            return None;
        }
        let i = self.scroll + ((y - l.rows_y) / ROW_H) as usize;
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
        let l = self.layout();
        // header buttons
        let on_btn = |bx: f32| {
            x >= bx - 2.0
                && x <= bx + BTN + 2.0
                && (l.btn_y - 2.0..=l.btn_y + BTN + 2.0).contains(&y)
        };
        if on_btn(l.btn_clear_x) {
            // two presses to clear everything: arm first, clear on the second
            self.clear_armed = !self.clear_armed;
            return Some(if self.clear_armed {
                PanelAction::ArmClear
            } else {
                PanelAction::Clear
            });
        }
        self.clear_armed = false;
        if on_btn(l.btn_close_x) {
            return Some(PanelAction::Close);
        }
        if on_btn(l.btn_lang_x) {
            return Some(PanelAction::ToggleLang);
        }
        if on_btn(l.btn_pause_x) {
            return Some(PanelAction::ToggleCapture);
        }
        if on_btn(l.btn_filter_x) {
            // pure panel state: cycle the source filter, no action needed
            self.cycle_source(store);
            return None;
        }
        // clip rows
        let visible = self.visible(store);
        if let Some(i) = self.row_at(y, visible.len()) {
            let id = visible[i].id;
            self.sel = i;
            if x < l.row_x + PIN_ZONE {
                return Some(PanelAction::TogglePin(id));
            }
            if x > l.row_x + l.row_w - DEL_ZONE {
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
        let rows = self.visible_rows();
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
                self.sel = self.sel.saturating_sub(rows);
                self.keep_sel_visible();
            }
            NavKey::PageDown => {
                self.sel = (self.sel + rows).min(total.saturating_sub(1));
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
            NavKey::Quick(n) => {
                // nth row from the very top of the filtered list (0-based,
                // matching the digit badges drawn on the rows)
                let visible = self.visible(store);
                if let Some(c) = visible.get(n as usize) {
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

    fn open_panel() -> Panel {
        Panel {
            open: true,
            ..Default::default()
        }
    }

    #[test]
    fn esc_peels_one_layer_per_press_in_spec_order() {
        // panel UI spec: Esc order is disarm clear -> clear query -> clear
        // source filter -> close, one layer per press.
        let mut p = open_panel();
        let s = store(40);
        p.cycle_source(&s); // (no sources here, so force one for the test)
        p.source = Some("Code".into());
        p.query = "fn".into();
        p.clear_armed = true;

        // 1. armed clear disarms (and nothing else changes)
        assert_eq!(p.nav(NavKey::Esc, &s), None);
        assert!(!p.clear_armed);
        assert_eq!(p.query, "fn");
        assert!(p.source.is_some());
        // 2. clears the query
        assert_eq!(p.nav(NavKey::Esc, &s), None);
        assert!(p.query.is_empty());
        assert!(p.source.is_some());
        // 3. clears the source filter
        assert_eq!(p.nav(NavKey::Esc, &s), None);
        assert!(p.source.is_none());
        // 4. closes the panel
        assert_eq!(p.nav(NavKey::Esc, &s), Some(PanelAction::Close));
    }

    #[test]
    fn page_and_home_end_navigation() {
        let mut p = open_panel();
        let s = store(40);
        let rows = p.visible_rows();
        p.nav(NavKey::PageDown, &s);
        assert_eq!(p.sel, rows, "PageDown jumps one viewport");
        p.nav(NavKey::Home, &s);
        assert_eq!(p.sel, 0);
        p.nav(NavKey::End, &s);
        assert_eq!(p.sel, p.visible(&s).len() - 1, "End selects the last row");
        p.nav(NavKey::PageUp, &s);
        assert_eq!(p.sel, p.visible(&s).len() - 1 - rows, "PageUp jumps back one viewport");
    }

    #[test]
    fn default_layout_matches_legacy_geometry() {
        // the original fixed panel: any drift here moves every click target
        let l = open_panel().layout();
        assert_eq!((l.canvas_w, l.canvas_h), (360.0, 542.0));
        assert_eq!(l.cat, (60.0, 286.0));
        assert_eq!((l.card_x, l.card_y, l.card_w, l.card_h), (4.0, 4.0, 352.0, 362.0));
        assert_eq!(l.btn_y, 10.0);
        assert_eq!(
            (l.btn_filter_x, l.btn_pause_x, l.btn_clear_x, l.btn_lang_x, l.btn_close_x),
            (242.0, 264.0, 286.0, 308.0, 330.0)
        );
        assert_eq!((l.search_x, l.search_y, l.search_w, l.search_h), (12.0, 34.0, 336.0, 20.0));
        assert_eq!((l.rows_y, l.row_x, l.row_w), (60.0, 12.0, 332.0));
        assert_eq!(l.rows, 8);
        assert_eq!(l.footer_y, 334.0);
    }

    #[test]
    fn click_routes_rows_and_zones() {
        let s = store(3);
        let mut p = open_panel();
        let l = p.layout();
        let ids: Vec<u64> = s.visible("").iter().map(|c| c.id).collect();
        // middle of first row copies
        let y0 = l.rows_y + ROW_H / 2.0;
        assert_eq!(p.click(150.0, y0, &s), Some(PanelAction::Copy(ids[0])));
        // pin zone
        assert_eq!(p.click(l.row_x + 5.0, y0, &s), Some(PanelAction::TogglePin(ids[0])));
        // delete zone
        assert_eq!(
            p.click(l.row_x + l.row_w - 5.0, y0, &s),
            Some(PanelAction::Delete(ids[0]))
        );
        // outside the card
        assert_eq!(p.click(150.0, l.card_y + l.card_h + 30.0, &s), None);
        // close button
        assert_eq!(
            p.click(l.btn_close_x + 8.0, l.btn_y + 8.0, &s),
            Some(PanelAction::Close)
        );
    }

    #[test]
    fn empty_row_area_clicks_do_nothing() {
        let s = store(1);
        let mut p = open_panel();
        let y = p.layout().rows_y + ROW_H * 3.5; // row 3, but only 1 clip
        assert_eq!(p.click(150.0, y, &s), None);
    }

    #[test]
    fn scroll_clamps() {
        let s = store(10);
        let mut p = open_panel();
        p.wheel(100, &s);
        assert_eq!(p.scroll, 10 - p.visible_rows());
        p.wheel(-100, &s);
        assert_eq!(p.scroll, 0);
    }

    #[test]
    fn keyboard_selection_follows_scroll() {
        let s = store(10);
        let mut p = open_panel();
        for _ in 0..8 {
            p.nav(NavKey::Down, &s);
        }
        assert_eq!(p.sel, 8);
        assert!(p.sel >= p.scroll && p.sel < p.scroll + p.visible_rows());
        let act = p.nav(NavKey::Enter, &s);
        let ids: Vec<u64> = s.visible("").iter().map(|c| c.id).collect();
        assert_eq!(act, Some(PanelAction::Copy(ids[8])));
    }

    #[test]
    fn quick_key_copies_nth_from_the_top() {
        let s = store(12);
        let mut p = open_panel();
        let ids: Vec<u64> = s.visible("").iter().map(|c| c.id).collect();
        assert_eq!(p.nav(NavKey::Quick(0), &s), Some(PanelAction::Copy(ids[0])));
        assert_eq!(p.nav(NavKey::Quick(9), &s), Some(PanelAction::Copy(ids[9])));
        // beyond the list: a no-op, not a panic
        let s3 = store(3);
        assert_eq!(p.nav(NavKey::Quick(5), &s3), None);
        // respects the active query (badge order = filtered order)
        p.input_char('7'); // "clip number 7"
        let id7 = s.visible("7")[0].id;
        assert_eq!(p.nav(NavKey::Quick(0), &s), Some(PanelAction::Copy(id7)));
        assert_eq!(p.nav(NavKey::Quick(1), &s), None);
    }

    #[test]
    fn drag_zones_move_and_resize_with_clamps() {
        let mut p = open_panel();
        let l = p.layout();
        // grip = resize, header strip = move, buttons/rows = no drag
        let (gx, gy) = (l.card_x + l.card_w - 4.0, l.card_y + l.card_h - 4.0);
        assert_eq!(p.drag_hit(gx, gy), Some(PanelDrag::Resize));
        assert_eq!(p.drag_hit(l.card_x + 60.0, l.card_y + 10.0), Some(PanelDrag::Move));
        assert_eq!(p.drag_hit(l.btn_close_x + 8.0, l.btn_y + 8.0), None);
        assert_eq!(p.drag_hit(l.row_x + 10.0, l.rows_y + 10.0), None);
        assert_eq!(p.drag_hit(l.search_x + 10.0, l.search_y + 10.0), None);
        // closed panel never drags
        let closed = Panel::default();
        assert_eq!(closed.drag_hit(gx, gy), None);

        // resize grows the card and the row count, clamped to MAX/MIN
        p.drag_by(PanelDrag::Resize, 60.0, 68.0);
        assert_eq!((p.w, p.h), (DEFAULT_W + 60.0, DEFAULT_H + 68.0));
        assert_eq!(p.layout().rows, 10);
        p.drag_by(PanelDrag::Resize, 9999.0, 9999.0);
        assert_eq!((p.w, p.h), (MAX_W, MAX_H));
        p.drag_by(PanelDrag::Resize, -9999.0, -9999.0);
        assert_eq!((p.w, p.h), (MIN_W, MIN_H));
        assert!(p.layout().rows >= 1);

        // move shifts the card offset, clamped; the canvas re-origins the cat
        p.drag_by(PanelDrag::Move, -30.0, -20.0);
        assert_eq!(p.off, (DEFAULT_OFF.0 - 30.0, DEFAULT_OFF.1 - 20.0));
        let l = p.layout();
        assert_eq!(l.cat, (-(p.off.0 - 4.0), -(p.off.1 - 4.0)));
        p.drag_by(PanelDrag::Move, -9999.0, 9999.0);
        assert_eq!(p.off, (-MAX_OFF, MAX_OFF));
    }

    #[test]
    fn geometry_clamp_rejects_garbage() {
        let (w, h, x, y) = clamp_geometry(f32::NAN, 1e9, -1e9, 1e9);
        assert_eq!(w, DEFAULT_W, "NaN falls back to the default");
        assert_eq!(h, MAX_H, "huge values clamp to the bounds");
        assert_eq!((x, y), (-MAX_OFF, MAX_OFF));
        let (w, h, _, _) = clamp_geometry(f32::INFINITY, f32::NEG_INFINITY, 0.0, 0.0);
        assert_eq!((w, h), (DEFAULT_W, DEFAULT_H), "non-finite falls back");
        assert_eq!(
            clamp_geometry(DEFAULT_W, DEFAULT_H, DEFAULT_OFF.0, DEFAULT_OFF.1),
            (DEFAULT_W, DEFAULT_H, DEFAULT_OFF.0, DEFAULT_OFF.1),
            "sane values pass through"
        );
    }

    #[test]
    fn card_can_travel_across_a_desktop() {
        // moving the card far away must not hit a clamp wall (the user can
        // park the panel anywhere on the desktop)
        let mut p = open_panel();
        p.drag_by(PanelDrag::Move, 1900.0, -1000.0);
        assert_eq!(p.off, (DEFAULT_OFF.0 + 1900.0, DEFAULT_OFF.1 - 1000.0));
        let l = p.layout();
        // the canvas stretches to keep both the cat and the far-away card
        assert!(l.canvas_w >= p.off.0 + p.w);
        assert!(l.cat.1 >= 1000.0);
        assert_eq!((l.card_x - l.cat.0, l.card_y - l.cat.1), p.off);
        // and back below/right of the cat works the same way
        p.off = DEFAULT_OFF;
        p.drag_by(PanelDrag::Move, 800.0, 1500.0);
        let l = p.layout();
        assert_eq!(l.cat, (0.0, 0.0), "card fully below-right: cat keeps the origin");
        assert!(l.canvas_h >= p.off.1 + p.h);
    }

    #[test]
    fn resize_keeps_scroll_in_range() {
        let s = store(20);
        let mut p = open_panel();
        p.nav(NavKey::End, &s);
        let bottom_scroll = p.scroll;
        assert!(bottom_scroll > 0);
        // a taller card shows more rows: scroll re-clamps on refresh
        p.drag_by(PanelDrag::Resize, 0.0, MAX_H);
        p.refresh(&s);
        assert!(p.scroll <= 20 - p.visible_rows());
        assert!(p.scroll < bottom_scroll);
        // selection still on screen
        assert!(p.sel >= p.scroll && p.sel < p.scroll + p.visible_rows());
    }

    #[test]
    fn esc_clears_query_then_filter_then_closes() {
        let mut s = store(2);
        s.add_copy("from chrome".into(), Some("Chrome".into()));
        let mut p = open_panel();
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
        let mut p = open_panel();
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
        let mut p = open_panel();
        let l = p.layout();
        assert_eq!(p.click(l.btn_filter_x + 8.0, l.btn_y + 8.0, &s), None);
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
        let mut p = open_panel();
        let l = p.layout();
        let (bx, by) = (l.btn_clear_x + 8.0, l.btn_y + 8.0);
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
        let mut p = open_panel();
        p.nav(NavKey::End, &s);
        assert_eq!(p.sel, 19);
        assert!(p.sel >= p.scroll && p.sel < p.scroll + p.visible_rows());
        p.nav(NavKey::Home, &s);
        assert_eq!((p.sel, p.scroll), (0, 0));
    }

    #[test]
    fn pin_and_undo_keys_act_on_selection() {
        let s = store(3);
        let mut p = open_panel();
        p.nav(NavKey::Down, &s);
        let id = s.visible("")[1].id;
        assert_eq!(p.nav(NavKey::Pin, &s), Some(PanelAction::TogglePin(id)));
        assert_eq!(p.nav(NavKey::Undo, &s), Some(PanelAction::Undo));
    }

    #[test]
    fn visible_cache_tracks_store_query_and_filter() {
        let mut s = store(3);
        let p = open_panel();
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
        let mut p = open_panel();
        p.wheel(5, &s);
        assert!(p.scroll > 0);
        p.input_char('7');
        assert_eq!(p.scroll, 0);
        assert_eq!(s.visible(&p.query).len(), 2); // "clip number 7", "...17"
    }
}
