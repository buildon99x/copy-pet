//! Clipboard history: the platform-agnostic clip store. Backends feed copied
//! text in via [`ClipStore::add_copy`] (Win32 clipboard listener on native,
//! arboard polling on portable) and ask for text to put back on the clipboard
//! when the user picks a clip from the panel. Persisted as JSON next to
//! `state.json`. Everything stays local — there is no network code.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Most unpinned clips kept in history (oldest evicted first).
pub const MAX_HISTORY: usize = 100;
/// Most pinned clips (pin attempts beyond this are ignored).
pub const MAX_PINNED: usize = 100;
/// Clips larger than this many bytes are ignored entirely (a clipboard pet
/// is not a paste-bin; truncating would corrupt a later paste).
pub const MAX_TEXT: usize = 256 * 1024;

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
}

#[derive(Serialize, Deserialize, Default)]
struct ClipsFile {
    clips: Vec<Clip>,
}

fn clips_file() -> Option<PathBuf> {
    crate::state::config_dir().map(|d| d.join("clips.json"))
}

impl ClipStore {
    pub fn load() -> ClipStore {
        let items = clips_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<ClipsFile>(&s).ok())
            .map(|f| f.clips)
            .unwrap_or_default();
        ClipStore::from_items(items)
    }

    fn from_items(items: Vec<Clip>) -> ClipStore {
        let next_id = items.iter().map(|c| c.id + 1).max().unwrap_or(1);
        ClipStore {
            items,
            next_id,
            dirty: false,
        }
    }

    pub fn save(&mut self) {
        if let (Some(dir), Some(file)) = (crate::state::config_dir(), clips_file()) {
            let _ = std::fs::create_dir_all(&dir);
            let f = ClipsFile {
                clips: self.items.clone(),
            };
            if let Ok(json) = serde_json::to_string(&f) {
                let _ = std::fs::write(file, json);
            }
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
        self.dirty = true;
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
            self.dirty = true;
        }
    }

    pub fn delete(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|c| c.id != id);
        let removed = self.items.len() != before;
        if removed {
            self.dirty = true;
        }
        removed
    }

    /// Removes all unpinned clips; returns how many were removed.
    pub fn clear_unpinned(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|c| c.pinned);
        let n = before - self.items.len();
        if n > 0 {
            self.dirty = true;
        }
        n
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
        let q = query.trim().to_lowercase();
        let matches = |c: &Clip| {
            if q.is_empty() {
                return true;
            }
            c.text.to_lowercase().contains(&q)
                || c.source
                    .as_deref()
                    .map(|s| s.to_lowercase().contains(&q))
                    .unwrap_or(false)
        };
        let mut out: Vec<&Clip> = self.items.iter().filter(|c| c.pinned && matches(c)).collect();
        out.extend(self.items.iter().filter(|c| !c.pinned && matches(c)));
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
        let json = serde_json::to_string(&ClipsFile {
            clips: s.items.clone(),
        })
        .unwrap();
        let back: ClipsFile = serde_json::from_str(&json).unwrap();
        let s2 = ClipStore::from_items(back.clips);
        assert_eq!(s2.len(), 2);
        assert_eq!(s2.pinned_count(), 1);
        assert!(s2.next_id > id);
    }
}
