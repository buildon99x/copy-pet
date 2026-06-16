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

pub const BTN: f32 = 18.0;
/// Per-clip row height in the compact list view (the default).
pub const ROW_H: f32 = 34.0;
/// Per-clip row height in the roomier rounded-box "thumbnail" view.
pub const ROW_H_THUMB: f32 = 52.0;
/// Left inset of the clip body text inside a row.
pub const BODY_X: f32 = 10.0;
/// Per-row right-edge gadget zones, measured leftward from the row's right edge.
/// Collapsed: the "..." overflow toggle then the pin star. Expanded (actions
/// revealed): two buttons, "paste as text" | "delete", each [`ACT_ZONE`] wide.
pub const OVF_ZONE: f32 = 22.0;
pub const PIN_ZONE: f32 = 22.0;
pub const ACT_ZONE: f32 = 24.0;
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

/// An axis-aligned rectangle. Used only by [`fit_delta`], so it carries no
/// coordinate-space assumptions — the backends pass screen pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// The smallest `(dx, dy)` that slides `card` fully inside `vis`, in whatever
/// units the two share. Zero when it already fits. When the card is larger
/// than `vis` on an axis its **start** edge (left/top) is aligned, so the
/// panel header, search box and top clips stay reachable rather than the
/// footer. The backends use this to pull a panel that opened off the monitor
/// back into view by moving the card (the cat stays anchored), so a pet near
/// a screen edge — or a card whose offset was dragged offscreen and persisted
/// — never hides the panel.
pub fn fit_delta(card: Rect, vis: Rect) -> (f32, f32) {
    (
        axis_fit(card.x, card.w, vis.x, vis.w),
        axis_fit(card.y, card.h, vis.y, vis.h),
    )
}

/// One axis of [`fit_delta`]: shift needed so `[pos, pos+len)` lies within
/// `[vmin, vmin+vlen)`, aligning the start when it can't fully fit.
fn axis_fit(pos: f32, len: f32, vmin: f32, vlen: f32) -> f32 {
    if len >= vlen || pos < vmin {
        vmin - pos
    } else if pos + len > vmin + vlen {
        vmin + vlen - pos - len
    } else {
        0.0
    }
}

/// Computed panel geometry for the current card size/offset, in **physical
/// pixels**. The canvas is the union of the cat block (240x256 scaled by the
/// cat's size) and the fixed-scale card (plus a margin); `cat` is where the
/// cat block sits inside it. Card-relative fields (`card_*`, `btn_*`,
/// `search_*`, `rows_*`, `row_*`, `footer_y`) are also physical pixels but,
/// since the card always renders at scale 1.0, they equal the card's own
/// canvas units.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    pub canvas_w: f32,
    pub canvas_h: f32,
    /// Top-left of the cat block inside the window canvas (physical pixels).
    pub cat: (f32, f32),
    pub card_x: f32,
    pub card_y: f32,
    pub card_w: f32,
    pub card_h: f32,
    /// Header buttons, right-aligned: view, filter, pause, clear, language, close.
    pub btn_y: f32,
    pub btn_close_x: f32,
    pub btn_lang_x: f32,
    pub btn_clear_x: f32,
    pub btn_pause_x: f32,
    pub btn_filter_x: f32,
    pub btn_view_x: f32,
    pub search_x: f32,
    pub search_y: f32,
    pub search_w: f32,
    pub search_h: f32,
    pub rows_y: f32,
    pub row_x: f32,
    pub row_w: f32,
    /// Per-row height (depends on the list/thumbnail view).
    pub row_h: f32,
    /// Clip rows that fit between the header and the footer.
    pub rows: usize,
    pub footer_y: f32,
}

/// What a panel interaction asks the app to do. Pure state changes
/// (scrolling, typing in search) are handled internally and return None.
#[derive(Debug, PartialEq)]
pub enum PanelAction {
    /// Put this clip's text back on the OS clipboard (formatting preserved).
    Copy(u64),
    /// Put this clip's *plain* text on the clipboard and paste it (formatting
    /// stripped) — the revealed "paste as text" row action (ADR-0014).
    PasteText(u64),
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
    /// Switch the clip list between the compact list and roomy cards.
    ToggleView,
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
    /// Paste the selected clip as plain text (Ctrl/Cmd+Enter): strips formatting.
    PasteText,
    /// Reveal the selected row's actions ("paste as text" / delete).
    Right,
    /// Collapse the selected row's revealed actions.
    Left,
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

pub struct Panel {
    pub open: bool,
    /// The panel is rendered in its own flyout window (hotkey path), so its
    /// geometry comes from the card-only [`layout_standalone`] canvas rather
    /// than the cat-union [`layout`]. Set by `Pet::open_flyout`, cleared by
    /// `Pet::close_flyout`; the embedded middle-click panel leaves it false.
    ///
    /// [`layout`]: Panel::layout
    /// [`layout_standalone`]: Panel::layout_standalone
    pub standalone: bool,
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
    /// Clip id whose inline actions ("paste as text" / delete) are revealed, if
    /// any. Only one row is expanded at a time; opened by the "..." overflow or
    /// the Right arrow, closed by Left/Esc or interacting with another row.
    pub expanded: Option<u64>,
    /// Card size in canvas units (user-resizable, persisted).
    pub w: f32,
    pub h: f32,
    /// Card top-left relative to the cat's top-left (user-movable, persisted).
    /// In panel units, i.e. physical pixels — the card always renders at scale
    /// 1.0 regardless of the cat's size (see [`Layout`]).
    pub off: (f32, f32),
    /// The cat's current size multiplier, mirrored from the Pet so [`layout`]
    /// can place the (fixed-scale) card next to the (scaled) cat. The card
    /// itself never uses this. Kept in sync by `Pet::set_scale_idx`.
    ///
    /// [`layout`]: Panel::layout
    pub cat_scale: f32,
    /// List style mirrored from `st.panel_view`: 0 = compact list, 1 = roomier
    /// rounded-box cards. Drives the per-row height (see [`Panel::row_h`]).
    pub view: u8,
    /// Cached filtered row order (the panel is redrawn every tick while
    /// open; without this the query would re-scan every clip text per frame).
    cache: RefCell<ViewCache>,
}

impl Default for Panel {
    fn default() -> Panel {
        Panel {
            open: false,
            standalone: false,
            query: String::new(),
            source: None,
            scroll: 0,
            sel: 0,
            cursor: None,
            clear_armed: false,
            expanded: None,
            w: DEFAULT_W,
            h: DEFAULT_H,
            off: DEFAULT_OFF,
            cat_scale: 1.0,
            view: 0,
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

    /// Per-clip row height for the active view (compact list vs. roomy cards).
    pub fn row_h(&self) -> f32 {
        if self.view == 1 {
            ROW_H_THUMB
        } else {
            ROW_H
        }
    }

    /// The full geometry for the current card size/offset, in **physical
    /// pixels**. The card always renders at scale 1.0 (panel units == physical
    /// pixels), while the cat block is sized by `cat_scale`; the canvas is the
    /// union of the two, so a larger cat grows the window only on the cat's
    /// side and the card keeps its fixed size and its `off` from the cat.
    pub fn layout(&self) -> Layout {
        let (w, h) = (self.w, self.h);
        let (off_x, off_y) = self.off;
        // physical size of the cat block (the card block is already 1.0)
        let cat_w = crate::render::CANVAS_W * self.cat_scale;
        let cat_h = crate::render::CANVAS_H * self.cat_scale;
        // canvas = union of the cat block and the margin-padded card
        let left = (off_x - MARGIN).min(0.0);
        let top = (off_y - MARGIN).min(0.0);
        let right = (off_x + w + MARGIN).max(cat_w);
        let bottom = (off_y + h + MARGIN).max(cat_h);
        let cat = (-left, -top);
        let card_x = cat.0 + off_x;
        let card_y = cat.1 + off_y;
        self.card_fields(right - left, bottom - top, cat, card_x, card_y)
    }

    /// Card-only layout for the flyout window (hotkey path): the card sits at
    /// `(MARGIN, MARGIN)` in its own canvas, sized to the card plus a margin
    /// on every side. The resize grip pokes ~2px past the card's bottom-right
    /// (`drag_hit`), which the 4px margin covers. The cat block is not part of
    /// this canvas; `cat` is set to the card origin and is unused here.
    pub fn layout_standalone(&self) -> Layout {
        let (w, h) = (self.w, self.h);
        let (card_x, card_y) = (MARGIN, MARGIN);
        self.card_fields(
            w + 2.0 * MARGIN,
            h + 2.0 * MARGIN,
            (card_x, card_y),
            card_x,
            card_y,
        )
    }

    /// The layout the panel is currently using: the card-only flyout canvas
    /// when it owns a separate window ([`standalone`]), else the cat-union
    /// canvas. Hit-testing, drawing and row math all go through this so the
    /// flyout's own client coords route exactly like the embedded panel.
    ///
    /// [`standalone`]: Panel::standalone
    pub fn active_layout(&self) -> Layout {
        if self.standalone {
            self.layout_standalone()
        } else {
            self.layout()
        }
    }

    /// The card-relative field block shared by [`layout`] (cat-union canvas)
    /// and [`layout_standalone`] (card-only flyout canvas): each computes the
    /// canvas size, cat origin and card origin its own way, then fills the
    /// rest here so the two geometries can never drift.
    ///
    /// [`layout`]: Panel::layout
    /// [`layout_standalone`]: Panel::layout_standalone
    fn card_fields(
        &self,
        canvas_w: f32,
        canvas_h: f32,
        cat: (f32, f32),
        card_x: f32,
        card_y: f32,
    ) -> Layout {
        let (w, h) = (self.w, self.h);
        let btn_close_x = card_x + w - 26.0;
        Layout {
            canvas_w,
            canvas_h,
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
            btn_view_x: btn_close_x - 110.0,
            search_x: card_x + 8.0,
            search_y: card_y + 30.0,
            search_w: w - 16.0,
            search_h: 20.0,
            rows_y: card_y + HEADER_H,
            row_x: card_x + 8.0,
            row_w: w - 20.0,
            row_h: self.row_h(),
            rows: (((h - HEADER_H - FOOTER_H) / self.row_h()) as usize).max(1),
            footer_y: card_y + h - FOOTER_H,
        }
    }

    /// Clip rows that fit on screen with the current card height.
    pub fn visible_rows(&self) -> usize {
        self.active_layout().rows
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
        self.expanded = None;
    }

    /// Collapses any revealed per-row actions.
    pub fn collapse_actions(&mut self) {
        self.expanded = None;
    }

    /// Zones that start a card drag: the bottom-right grip resizes, the
    /// header strip left of the buttons moves the card.
    pub fn drag_hit(&self, x: f32, y: f32) -> Option<PanelDrag> {
        if !self.open {
            return None;
        }
        let l = self.active_layout();
        let (x1, y1) = (l.card_x + l.card_w, l.card_y + l.card_h);
        if x >= x1 - GRIP && x <= x1 + 2.0 && y >= y1 - GRIP && y <= y1 + 2.0 {
            return Some(PanelDrag::Resize);
        }
        if self.hit(x, y) && y < l.search_y - 2.0 && x < l.btn_view_x - 4.0 {
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
        // the list changed under the panel (delete/clear/undo/resize): close any
        // revealed action row so it can't point at a stale clip.
        self.expanded = None;
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
        self.expanded = None;
    }

    /// True if the point (canvas coords) is inside the panel card.
    pub fn hit(&self, x: f32, y: f32) -> bool {
        if !self.open {
            return false;
        }
        let l = self.active_layout();
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
        let l = self.active_layout();
        if y < l.rows_y || y >= l.rows_y + l.row_h * l.rows as f32 {
            return None;
        }
        let i = self.scroll + ((y - l.rows_y) / l.row_h) as usize;
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
        let l = self.active_layout();
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
        if on_btn(l.btn_view_x) {
            return Some(PanelAction::ToggleView);
        }
        // clip rows
        let visible = self.visible(store);
        if let Some(i) = self.row_at(y, visible.len()) {
            let id = visible[i].id;
            self.sel = i;
            let right = l.row_x + l.row_w;
            if self.expanded == Some(id) {
                // revealed actions: [ paste as text ] [ delete ]
                if x > right - ACT_ZONE {
                    return Some(PanelAction::Delete(id));
                }
                if x > right - 2.0 * ACT_ZONE {
                    return Some(PanelAction::PasteText(id));
                }
                // body of an expanded row: copy it and collapse the actions
                self.expanded = None;
                return Some(PanelAction::Copy(id));
            }
            // collapsed: [ ... overflow ] [ pin ] | body
            if x > right - OVF_ZONE {
                self.expanded = Some(id); // reveal this row's actions (pure state)
                return None;
            }
            // any other row interaction closes a menu open on another row
            self.expanded = None;
            if x > right - OVF_ZONE - PIN_ZONE {
                return Some(PanelAction::TogglePin(id));
            }
            return Some(PanelAction::Copy(id));
        }
        self.expanded = None; // click in the empty row area closes an open menu
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
        // Right reveals the selected row's actions; Esc peels them in its own
        // handler; every other key collapses an open action row.
        if !matches!(key, NavKey::Right | NavKey::Esc) {
            self.expanded = None;
        }
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
            NavKey::PasteText => {
                // mouse-free "paste as text": strip formatting on the selection
                let visible = self.visible(store);
                if let Some(c) = visible.get(self.sel) {
                    return Some(PanelAction::PasteText(c.id));
                }
            }
            NavKey::Right => {
                // reveal the selected row's actions (pure state)
                let visible = self.visible(store);
                if let Some(c) = visible.get(self.sel) {
                    self.expanded = Some(c.id);
                }
                return None;
            }
            NavKey::Left => return None, // actions already collapsed above
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
                // peel back one layer at a time: armed clear, revealed actions,
                // query, filter, close
                if armed {
                    return None;
                }
                if self.expanded.take().is_some() {
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
        // middle of first row copies (formatting preserved)
        let y0 = l.rows_y + ROW_H / 2.0;
        let right = l.row_x + l.row_w;
        assert_eq!(p.click(150.0, y0, &s), Some(PanelAction::Copy(ids[0])));
        // pin zone now sits on the right, just left of the "..." overflow
        assert_eq!(
            p.click(right - OVF_ZONE - 5.0, y0, &s),
            Some(PanelAction::TogglePin(ids[0]))
        );
        // the "..." overflow reveals the row's actions (no action emitted)
        assert_eq!(p.click(right - 4.0, y0, &s), None);
        assert_eq!(p.expanded, Some(ids[0]));
        // revealed: rightmost button deletes, the one left of it pastes as text
        assert_eq!(p.click(right - 4.0, y0, &s), Some(PanelAction::Delete(ids[0])));
        assert_eq!(
            p.click(right - ACT_ZONE - 4.0, y0, &s),
            Some(PanelAction::PasteText(ids[0]))
        );
        // a click on the body of an expanded row copies it and collapses
        assert_eq!(p.click(150.0, y0, &s), Some(PanelAction::Copy(ids[0])));
        assert_eq!(p.expanded, None);
        // outside the card
        assert_eq!(p.click(150.0, l.card_y + l.card_h + 30.0, &s), None);
        // close button
        assert_eq!(
            p.click(l.btn_close_x + 8.0, l.btn_y + 8.0, &s),
            Some(PanelAction::Close)
        );
        // view-toggle button (leftmost of the header cluster)
        assert_eq!(
            p.click(l.btn_view_x + 8.0, l.btn_y + 8.0, &s),
            Some(PanelAction::ToggleView)
        );
        // the view button is not part of the move-drag handle
        assert_eq!(p.drag_hit(l.btn_view_x + 8.0, l.btn_y + 8.0), None);
        // the header strip left of it still moves the card
        assert_eq!(p.drag_hit(l.btn_view_x - 12.0, l.btn_y + 8.0), Some(PanelDrag::Move));
    }

    #[test]
    fn arrow_keys_and_esc_drive_the_action_row() {
        let s = store(3);
        let mut p = open_panel();
        let ids: Vec<u64> = s.visible("").iter().map(|c| c.id).collect();
        // Right reveals the selected row's actions; Left collapses them
        assert_eq!(p.nav(NavKey::Right, &s), None);
        assert_eq!(p.expanded, Some(ids[0]));
        assert_eq!(p.nav(NavKey::Left, &s), None);
        assert_eq!(p.expanded, None);
        // moving the selection collapses an open action row
        p.nav(NavKey::Right, &s);
        p.nav(NavKey::Down, &s);
        assert_eq!(p.expanded, None);
        // Esc peels the actions before it would close the panel
        p.nav(NavKey::Right, &s);
        assert!(p.expanded.is_some());
        assert_eq!(p.nav(NavKey::Esc, &s), None);
        assert_eq!(p.expanded, None, "esc collapses the actions first");
        assert_eq!(p.nav(NavKey::Esc, &s), Some(PanelAction::Close));
    }

    #[test]
    fn ctrl_enter_pastes_selected_as_text() {
        let s = store(3);
        let mut p = open_panel();
        let ids: Vec<u64> = s.visible("").iter().map(|c| c.id).collect();
        p.nav(NavKey::Down, &s); // select row 1
        assert_eq!(p.nav(NavKey::PasteText, &s), Some(PanelAction::PasteText(ids[1])));
        // it works without opening the "..." menu, and collapses any open one
        assert_eq!(p.expanded, None);
        // empty list: a no-op, not a panic
        let empty = ClipStore::default();
        assert_eq!(p.nav(NavKey::PasteText, &empty), None);
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
    fn fit_delta_pulls_an_offscreen_card_back() {
        // a 1000x800 monitor at the origin
        let vis = Rect { x: 0.0, y: 0.0, w: 1000.0, h: 800.0 };
        let card = |x, y| Rect { x, y, w: 300.0, h: 400.0 };
        // already inside: no move
        assert_eq!(fit_delta(card(100.0, 100.0), vis), (0.0, 0.0));
        // off the top-left: pushed down-right by the overflow
        assert_eq!(fit_delta(card(-40.0, -60.0), vis), (40.0, 60.0));
        // off the bottom-right: pulled up-left so the far edge lands on vis
        assert_eq!(fit_delta(card(800.0, 600.0), vis), (-100.0, -200.0));
        // only one axis offscreen
        assert_eq!(fit_delta(card(-10.0, 300.0), vis), (10.0, 0.0));
    }

    #[test]
    fn fit_delta_aligns_start_when_card_exceeds_viewport() {
        // a short viewport the tall card can't fit inside
        let vis = Rect { x: 50.0, y: 20.0, w: 1000.0, h: 300.0 };
        let card = Rect { x: 100.0, y: 100.0, w: 300.0, h: 400.0 };
        // height exceeds vis: align the card top to vis top (header stays on
        // screen), x already fits
        assert_eq!(fit_delta(card, vis), (0.0, -80.0));
    }

    #[test]
    fn thumbnail_view_uses_taller_rows_and_hit_tests_them() {
        let s = store(6);
        let mut p = open_panel();
        let list = p.layout();
        assert_eq!(list.row_h, ROW_H);
        p.view = 1;
        let cards = p.layout();
        assert_eq!(cards.row_h, ROW_H_THUMB);
        assert!(cards.rows < list.rows, "taller cards => fewer fit on screen");
        // row_at honors the active (taller) row height
        let y = cards.rows_y + ROW_H_THUMB * 1.5;
        assert_eq!(p.row_at(y, 6), Some(1));
        // a click in that card's body still copies it
        let act = p.click(cards.row_x + 60.0, y, &s);
        assert!(matches!(act, Some(PanelAction::Copy(_))));
    }

    #[test]
    fn layout_scales_only_the_cat_block() {
        let mut p = open_panel();
        let base = p.layout();
        p.cat_scale = 1.3; // grow the cat
        let big = p.layout();
        // the card never scales: its placement and size are invariant
        assert_eq!(
            (big.card_x, big.card_y, big.card_w, big.card_h),
            (base.card_x, base.card_y, base.card_w, base.card_h),
        );
        // the union grows only because the cat block grew
        assert!(big.canvas_w >= base.canvas_w && big.canvas_h >= base.canvas_h);
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

    #[test]
    fn standalone_layout_is_card_only_at_margin() {
        let p = open_panel();
        let l = p.layout_standalone();
        // card sits at MARGIN; canvas is just the card plus a margin all round
        assert_eq!((l.card_x, l.card_y), (MARGIN, MARGIN));
        assert_eq!((l.card_w, l.card_h), (DEFAULT_W, DEFAULT_H));
        assert_eq!((l.canvas_w, l.canvas_h), (DEFAULT_W + 8.0, DEFAULT_H + 8.0));
        // the resize grip pokes ~2px past the card's bottom-right; the margin
        // must cover it so it stays inside the flyout canvas
        let (gx, gy) = (l.card_x + l.card_w, l.card_y + l.card_h);
        assert!(gx + 2.0 <= l.canvas_w && gy + 2.0 <= l.canvas_h);
        // rows count only depends on the card height, so it matches the union
        assert_eq!(l.rows, p.layout().rows);
    }

    #[test]
    fn active_layout_follows_standalone_flag() {
        let mut p = open_panel();
        assert_eq!(p.active_layout(), p.layout());
        p.standalone = true;
        assert_eq!(p.active_layout(), p.layout_standalone());
    }

    #[test]
    fn standalone_hit_test_and_click_route_like_the_union() {
        // a click at the same *card-relative* point must hit the same row in
        // either layout, since both feed the same card_fields math
        let s = store(3);
        let ids: Vec<u64> = s.visible("").iter().map(|c| c.id).collect();

        let mut flo = open_panel();
        flo.standalone = true;
        let l = flo.active_layout();
        assert!(flo.hit(l.card_x + 5.0, l.card_y + 5.0));
        assert!(!flo.hit(0.0, 0.0)); // the margin around the card is not the card
        let y0 = l.rows_y + ROW_H / 2.0;
        assert_eq!(flo.click(l.row_x + l.row_w / 2.0, y0, &s), Some(PanelAction::Copy(ids[0])));
        // the grip is still reachable in the flyout's own coords
        assert_eq!(
            flo.drag_hit(l.card_x + l.card_w - 2.0, l.card_y + l.card_h - 2.0),
            Some(PanelDrag::Resize)
        );
    }
}
