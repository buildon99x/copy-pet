//! Clipboard history: the platform-agnostic clip store. Backends feed copied
//! text in via [`ClipStore::add_copy`] (Win32 clipboard listener on native,
//! arboard polling on portable) and ask for text to put back on the clipboard
//! when the user picks a clip from the panel. Persisted as JSON next to
//! `state.json`. Everything stays local — there is no network code.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Most unpinned clips kept in history (oldest evicted first).
pub const MAX_HISTORY: usize = 100;
/// Most pinned clips (pin attempts beyond this are ignored).
pub const MAX_PINNED: usize = 100;
/// Clips larger than this many bytes are ignored entirely (a clipboard pet
/// is not a paste-bin; truncating would corrupt a later paste).
pub const MAX_TEXT: usize = 256 * 1024;
/// Most delete operations kept for undo (session-only, not persisted).
const MAX_UNDO: usize = 20;

#[derive(Serialize, Deserialize, Clone)]
pub struct Clip {
    pub id: u64,
    pub text: String,
    /// Source application name (e.g. "Code", "chrome"), when the backend
    /// could determine it. Used for the fish badge and the row meta line.
    pub source: Option<String>,
    pub pinned: bool,
    /// Unix seconds of the last copy of this text.
    pub ts: u64,
}

impl Clip {
    /// Single-line preview: first non-empty line, whitespace collapsed.
    pub fn preview(&self) -> String {
        let line = self
            .text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        let mut out = String::new();
        let mut last_space = false;
        for c in line.trim().chars().take(120) {
            if c.is_whitespace() {
                if !last_space {
                    out.push(' ');
                }
                last_space = true;
            } else {
                out.push(c);
                last_space = false;
            }
        }
        out
    }
}

pub fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Default)]
pub struct ClipStore {
    /// Newest first.
    items: Vec<Clip>,
    next_id: u64,
    pub dirty: bool,
    /// Recently deleted clips, one entry per delete/clear operation, newest
    /// last. Session-only: lets the panel undo an accidental delete.
    undo: Vec<Vec<Clip>>,
    /// Monotonic mutation counter; bumps whenever the list could look
    /// different, so the panel's cached filtered view knows to recompute.
    version: u64,
    /// Set when `load()` found a corrupt `clips.json`, backed it up and started
    /// fresh — so the UI can toast it once (panel error-UX spec). Runtime-only.
    recovered_corrupt: bool,
}

#[derive(Deserialize, Default)]
struct ClipsFile {
    clips: Vec<Clip>,
}

/// Borrowing twin of [`ClipsFile`] so saving doesn't clone every clip.
#[derive(Serialize)]
struct ClipsOut<'a> {
    clips: &'a [Clip],
}

/// Case-folded chars of `s` (per-char lowercase, no allocation).
fn fold(s: &str) -> impl Iterator<Item = char> + '_ {
    s.chars().flat_map(char::to_lowercase)
}

/// Allocation-free case-insensitive string equality.
fn eq_ci(a: &str, b: &str) -> bool {
    fold(a).eq(fold(b))
}

/// Allocation-free case-insensitive substring test; the panel search runs
/// this against every clip, so it must not lowercase whole clip texts into
/// fresh `String`s.
fn contains_ci(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut start = hay.char_indices();
    loop {
        let mut h = fold(start.as_str());
        let mut n = fold(needle);
        loop {
            match (n.next(), h.next()) {
                (None, _) => return true,
                (Some(nc), Some(hc)) if nc == hc => continue,
                _ => break,
            }
        }
        if start.next().is_none() {
            return false;
        }
    }
}

fn clips_file() -> Option<PathBuf> {
    crate::state::config_dir().map(|d| d.join("clips.json"))
}

impl ClipStore {
    pub fn load() -> ClipStore {
        match clips_file() {
            Some(p) => ClipStore::load_from(&p),
            None => ClipStore::from_items(Vec::new()),
        }
    }

    /// Loads the history from a specific file. A missing/unreadable file is
    /// just first-run (start empty); an *existing* file that won't parse is
    /// corrupt — rename it to `clips.corrupt.<ts>.json` for forensics, start
    /// fresh, and flag a one-time toast (panel error-UX spec).
    fn load_from(path: &Path) -> ClipStore {
        let Ok(text) = std::fs::read_to_string(path) else {
            return ClipStore::from_items(Vec::new());
        };
        match serde_json::from_str::<ClipsFile>(&text) {
            Ok(f) => ClipStore::from_items(f.clips),
            Err(_) => {
                let backup = path.with_file_name(format!("clips.corrupt.{}.json", now_ts()));
                let _ = std::fs::rename(path, &backup);
                let mut s = ClipStore::from_items(Vec::new());
                s.recovered_corrupt = true;
                s
            }
        }
    }

    /// True once after a corrupt `clips.json` was recovered at load; the value
    /// is taken so the toast fires a single time.
    pub fn take_recovered_corrupt(&mut self) -> bool {
        std::mem::take(&mut self.recovered_corrupt)
    }

    fn from_items(items: Vec<Clip>) -> ClipStore {
        let next_id = items.iter().map(|c| c.id + 1).max().unwrap_or(1);
        ClipStore {
            items,
            next_id,
            dirty: false,
            undo: Vec::new(),
            version: 0,
            recovered_corrupt: false,
        }
    }

    /// Monotonic mutation counter (see the field doc).
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Writes the history to disk atomically (see [`crate::state::write_atomic`]).
    pub fn save(&mut self) {
        let out = ClipsOut { clips: &self.items };
        if let (Some(file), Ok(json)) = (clips_file(), serde_json::to_string(&out)) {
            crate::state::write_atomic(&file, &json);
        }
        self.dirty = false;
    }

    pub fn save_if_dirty(&mut self) {
        if self.dirty {
            self.save();
        }
    }

    /// Records a copy event. Returns `true` when it was accepted (new clip or
    /// an existing one bumped to the top) so the pet can react.
    pub fn add_copy(&mut self, text: String, source: Option<String>) -> bool {
        if text.trim().is_empty() || text.len() > MAX_TEXT {
            return false;
        }
        let ts = now_ts();
        if let Some(i) = self.items.iter().position(|c| c.text == text) {
            // same text copied again: bump to top, refresh meta
            let mut clip = self.items.remove(i);
            clip.ts = ts;
            if source.is_some() {
                clip.source = source;
            }
            self.items.insert(0, clip);
        } else {
            let clip = Clip {
                id: self.next_id,
                text,
                source,
                pinned: false,
                ts,
            };
            self.next_id += 1;
            self.items.insert(0, clip);
            self.evict();
        }
        self.touch();
        true
    }

    fn evict(&mut self) {
        let mut unpinned = self.items.iter().filter(|c| !c.pinned).count();
        while unpinned > MAX_HISTORY {
            if let Some(i) = self.items.iter().rposition(|c| !c.pinned) {
                self.items.remove(i);
                unpinned -= 1;
            } else {
                break;
            }
        }
    }

    pub fn get(&self, id: u64) -> Option<&Clip> {
        self.items.iter().find(|c| c.id == id)
    }

    /// Toggles a pin; refuses to pin beyond [`MAX_PINNED`].
    pub fn toggle_pin(&mut self, id: u64) {
        let pinned_count = self.pinned_count();
        if let Some(c) = self.items.iter_mut().find(|c| c.id == id) {
            if !c.pinned && pinned_count >= MAX_PINNED {
                return;
            }
            c.pinned = !c.pinned;
            self.touch();
        }
    }

    pub fn delete(&mut self, id: u64) -> bool {
        let Some(i) = self.items.iter().position(|c| c.id == id) else {
            return false;
        };
        let clip = self.items.remove(i);
        self.push_undo(vec![clip]);
        self.touch();
        true
    }

    /// Removes all unpinned clips; returns how many were removed.
    pub fn clear_unpinned(&mut self) -> usize {
        let (kept, removed): (Vec<Clip>, Vec<Clip>) =
            self.items.drain(..).partition(|c| c.pinned);
        self.items = kept;
        let n = removed.len();
        if n > 0 {
            self.push_undo(removed);
            self.touch();
        }
        n
    }

    /// Marks the store changed: needs saving, and cached views are stale.
    fn touch(&mut self) {
        self.dirty = true;
        self.version += 1;
    }

    fn push_undo(&mut self, batch: Vec<Clip>) {
        if self.undo.len() >= MAX_UNDO {
            self.undo.remove(0);
        }
        self.undo.push(batch);
    }

    /// Restores the most recent delete/clear operation. Returns the id of one
    /// restored clip (for the panel to re-select), or None if nothing to undo.
    pub fn undo_delete(&mut self) -> Option<u64> {
        let batch = self.undo.pop()?;
        let first = batch.first().map(|c| c.id);
        for clip in batch {
            // re-insert at the ts-sorted spot (items are newest first)
            let i = self
                .items
                .iter()
                .position(|c| c.ts <= clip.ts)
                .unwrap_or(self.items.len());
            self.items.insert(i, clip);
        }
        self.evict();
        self.touch();
        first
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn pinned_count(&self) -> usize {
        self.items.iter().filter(|c| c.pinned).count()
    }

    /// Clips for the panel: pinned first (newest first), then history
    /// (newest first), filtered by a case-insensitive substring query on
    /// text and source app.
    pub fn visible(&self, query: &str) -> Vec<&Clip> {
        self.visible_filtered(query, None)
    }

    /// Like [`ClipStore::visible`], additionally restricted to clips whose
    /// source app equals `source` (case-insensitive), when given.
    pub fn visible_filtered(&self, query: &str, source: Option<&str>) -> Vec<&Clip> {
        self.filtered_indices(query, source)
            .into_iter()
            .map(|i| &self.items[i])
            .collect()
    }

    /// The row order behind [`ClipStore::visible_filtered`], as indices into
    /// the store. The panel caches this (keyed on [`ClipStore::version`])
    /// so the filter doesn't re-run over every clip text each frame.
    pub fn filtered_indices(&self, query: &str, source: Option<&str>) -> Vec<usize> {
        let q = query.trim();
        let matches = |c: &Clip| {
            if let Some(want) = source {
                if !c.source.as_deref().is_some_and(|s| eq_ci(s, want)) {
                    return false;
                }
            }
            q.is_empty()
                || contains_ci(&c.text, q)
                || c.source.as_deref().is_some_and(|s| contains_ci(s, q))
        };
        let row = |want_pinned: bool| {
            self.items
                .iter()
                .enumerate()
                .filter(move |(_, c)| c.pinned == want_pinned && matches(c))
                .map(|(i, _)| i)
        };
        let mut out: Vec<usize> = row(true).collect();
        out.extend(row(false));
        out
    }

    /// The clip at a [`ClipStore::filtered_indices`] position, if still valid.
    pub fn by_index(&self, i: usize) -> Option<&Clip> {
        self.items.get(i)
    }

    /// Distinct source apps across the history, most recently used first
    /// (case-insensitive dedupe, first spelling wins). Drives the panel's
    /// source-filter cycle button.
    pub fn sources(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for c in &self.items {
            if let Some(s) = c.source.as_deref() {
                if !out.iter().any(|seen| eq_ci(seen, s)) {
                    out.push(s);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(texts: &[&str]) -> ClipStore {
        let mut s = ClipStore::default();
        s.next_id = 1;
        for t in texts {
            s.add_copy(t.to_string(), None);
        }
        s
    }

    #[test]
    fn corrupt_history_is_backed_up_and_reset() {
        let dir = std::env::temp_dir().join(format!("clipcat-corrupt-{}", now_ts()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clips.json");
        std::fs::write(&path, b"{ this is not valid json ]]").unwrap();

        let mut s = ClipStore::load_from(&path);
        assert!(s.is_empty(), "a corrupt history starts fresh");
        assert!(s.take_recovered_corrupt(), "and flags the one-time toast");
        assert!(!s.take_recovered_corrupt(), "flag is taken, fires only once");

        // the bad file was preserved (renamed), not silently dropped
        assert!(!path.exists(), "the corrupt file was moved aside");
        let backed_up = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("clips.corrupt."));
        assert!(backed_up, "a clips.corrupt.* backup exists");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn valid_history_loads_without_recovery() {
        let dir = std::env::temp_dir().join(format!("clipcat-ok-{}", now_ts()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clips.json");
        std::fs::write(&path, br#"{"clips":[]}"#).unwrap();
        let mut s = ClipStore::load_from(&path);
        assert!(!s.take_recovered_corrupt());
        // a missing file is first-run, not corruption
        let mut fresh = ClipStore::load_from(&dir.join("nope.json"));
        assert!(!fresh.take_recovered_corrupt());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_orders_newest_first() {
        let s = store_with(&["one", "two", "three"]);
        let v = s.visible("");
        assert_eq!(v[0].text, "three");
        assert_eq!(v[2].text, "one");
    }

    #[test]
    fn duplicate_copy_bumps_to_top() {
        let mut s = store_with(&["one", "two"]);
        assert!(s.add_copy("one".into(), Some("editor".into())));
        let v = s.visible("");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].text, "one");
        assert_eq!(v[0].source.as_deref(), Some("editor"));
    }

    #[test]
    fn rejects_empty_and_huge() {
        let mut s = ClipStore::default();
        assert!(!s.add_copy("   \n ".into(), None));
        assert!(!s.add_copy("x".repeat(MAX_TEXT + 1), None));
        assert!(s.is_empty());
    }

    #[test]
    fn eviction_spares_pinned() {
        let mut s = ClipStore::default();
        s.add_copy("keep me".into(), None);
        let id = s.visible("")[0].id;
        s.toggle_pin(id);
        for i in 0..(MAX_HISTORY + 20) {
            s.add_copy(format!("clip {i}"), None);
        }
        assert!(s.get(id).is_some(), "pinned clip evicted");
        assert_eq!(s.len(), MAX_HISTORY + 1);
    }

    #[test]
    fn pinned_sort_first_in_visible() {
        let mut s = store_with(&["a", "b", "c"]);
        let b_id = s.visible("").iter().find(|c| c.text == "b").unwrap().id;
        s.toggle_pin(b_id);
        let v = s.visible("");
        assert_eq!(v[0].text, "b");
        assert!(v[0].pinned);
    }

    #[test]
    fn search_filters_text_and_source() {
        let mut s = ClipStore::default();
        s.add_copy("Hello World".into(), Some("Chrome".into()));
        s.add_copy("안녕하세요".into(), Some("Code".into()));
        assert_eq!(s.visible("hello").len(), 1);
        assert_eq!(s.visible("안녕").len(), 1);
        assert_eq!(s.visible("chrome").len(), 1);
        assert_eq!(s.visible("zzz").len(), 0);
        assert_eq!(s.visible("").len(), 2);
    }

    #[test]
    fn source_filter_and_distinct_sources() {
        let mut s = ClipStore::default();
        s.add_copy("one".into(), Some("Chrome".into()));
        s.add_copy("two".into(), Some("Code".into()));
        s.add_copy("three".into(), Some("chrome".into())); // dup, different case
        s.add_copy("four".into(), None);
        // distinct, most recent first, first spelling kept per app
        assert_eq!(s.sources(), vec!["chrome", "Code"]);
        // filter is case-insensitive equality on the source
        let chrome = s.visible_filtered("", Some("CHROME"));
        assert_eq!(chrome.len(), 2);
        assert!(chrome.iter().all(|c| c.source.as_deref().unwrap().eq_ignore_ascii_case("chrome")));
        // sourceless clips only match without a filter
        assert_eq!(s.visible_filtered("four", Some("chrome")).len(), 0);
        assert_eq!(s.visible_filtered("four", None).len(), 1);
        // query and filter combine
        assert_eq!(s.visible_filtered("one", Some("chrome")).len(), 1);
        assert_eq!(s.visible_filtered("two", Some("chrome")).len(), 0);
    }

    #[test]
    fn delete_and_clear() {
        let mut s = store_with(&["a", "b", "c"]);
        let a_id = s.visible("").iter().find(|c| c.text == "a").unwrap().id;
        s.toggle_pin(a_id);
        let c_id = s.visible("").iter().find(|c| c.text == "c").unwrap().id;
        assert!(s.delete(c_id));
        assert_eq!(s.clear_unpinned(), 1); // "b"
        assert_eq!(s.len(), 1);
        assert!(s.get(a_id).is_some());
    }

    #[test]
    fn undo_restores_deletes_in_reverse_order() {
        let mut s = store_with(&["a", "b", "c"]);
        let b_id = s.visible("").iter().find(|c| c.text == "b").unwrap().id;
        let c_id = s.visible("").iter().find(|c| c.text == "c").unwrap().id;
        s.delete(b_id);
        s.delete(c_id);
        assert_eq!(s.len(), 1);
        // last deleted comes back first, at its old (newest-first) spot
        assert_eq!(s.undo_delete(), Some(c_id));
        assert_eq!(s.visible("")[0].text, "c");
        assert_eq!(s.undo_delete(), Some(b_id));
        assert_eq!(s.len(), 3);
        assert_eq!(s.undo_delete(), None, "nothing left to undo");
    }

    #[test]
    fn undo_restores_a_whole_clear() {
        let mut s = store_with(&["a", "b", "c"]);
        let a_id = s.visible("").iter().find(|c| c.text == "a").unwrap().id;
        s.toggle_pin(a_id);
        assert_eq!(s.clear_unpinned(), 2);
        assert_eq!(s.len(), 1);
        assert!(s.undo_delete().is_some());
        assert_eq!(s.len(), 3, "clear is undone as one operation");
    }

    #[test]
    fn search_is_case_insensitive_without_allocs() {
        assert!(contains_ci("Hello World", "WORLD"));
        assert!(contains_ci("HELLO", "hello"));
        assert!(contains_ci("안녕하세요", "녕하"));
        assert!(contains_ci("mixed 한글 Text", "한글 t"));
        assert!(!contains_ci("Hello", "World"));
        assert!(!contains_ci("", "x"));
        assert!(contains_ci("anything", ""));
    }

    #[test]
    fn preview_collapses_whitespace() {
        let c = Clip {
            id: 1,
            text: "\n\n  fn   main() {\n    body\n}".into(),
            source: None,
            pinned: false,
            ts: 0,
        };
        assert_eq!(c.preview(), "fn main() {");
    }

    #[test]
    fn round_trips_through_json() {
        let mut s = store_with(&["alpha", "베타"]);
        let id = s.visible("")[0].id;
        s.toggle_pin(id);
        let json = serde_json::to_string(&ClipsOut { clips: &s.items }).unwrap();
        let back: ClipsFile = serde_json::from_str(&json).unwrap();
        let s2 = ClipStore::from_items(back.clips);
        assert_eq!(s2.len(), 2);
        assert_eq!(s2.pinned_count(), 1);
        assert!(s2.next_id > id);
    }
}
