//! Clipboard history: the platform-agnostic clip store. Backends feed copied
//! text in via [`ClipStore::add_copy`] (Win32 clipboard listener on native,
//! arboard polling on portable) and ask for text to put back on the clipboard
//! when the user picks a clip from the panel. Persisted as JSON next to
//! `state.json`. Everything stays local — there is no network code.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Most unpinned clips kept in history (oldest evicted first).
pub const MAX_HISTORY: usize = 100;
/// Most pinned clips (pin attempts beyond this are ignored).
pub const MAX_PINNED: usize = 100;
/// Clips larger than this many bytes are ignored entirely (a clipboard pet
/// is not a paste-bin; truncating would corrupt a later paste).
pub const MAX_TEXT: usize = 256 * 1024;
/// Cap on a clip's optional rich formats (HTML + base64 RTF), independent of
/// [`MAX_TEXT`]. On overflow the rich formats are dropped but the plain clip is
/// kept — a paste still works, it just loses the formatting (ADR-0014).
pub const MAX_RICH: usize = 1024 * 1024;
/// Most delete operations kept for undo (session-only, not persisted).
const MAX_UNDO: usize = 20;

/// Optional rich clipboard formats kept alongside a clip's plain [`Clip::text`]
/// (ADR-0014). Opaque to the core — backends encode/decode the OS formats. The
/// plain text is always the source of truth for search, preview and de-dupe;
/// these are only re-emitted when the user pastes with formatting preserved.
/// Captured on Windows + macOS; Linux leaves them `None` (arboard is text-only).
#[derive(Serialize, Deserialize, Clone, Default, PartialEq, Eq, Debug)]
pub struct RichFormats {
    /// CF_HTML (Windows) / `public.html` (macOS) blob, stored verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    /// RTF bytes, base64-encoded (serde_json bloats raw `Vec<u8>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtf_b64: Option<String>,
}

impl RichFormats {
    pub fn is_empty(&self) -> bool {
        self.html.is_none() && self.rtf_b64.is_none()
    }

    /// Total encoded size, for the [`MAX_RICH`] cap.
    fn byte_len(&self) -> usize {
        self.html.as_ref().map_or(0, String::len) + self.rtf_b64.as_ref().map_or(0, String::len)
    }
}

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
    /// Original rich formats (HTML/RTF), when the backend captured them and the
    /// platform supports them. `None` for old clips, plain copies and Linux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formats: Option<RichFormats>,
}

// ---- base64 (inline; no crate, per the "no heavy deps" golden rule) ---------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 (with `=` padding). Used by backends to stash RTF bytes in
/// [`RichFormats::rtf_b64`] without bloating the JSON into an integer array.
pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(n >> 18 & 63) as usize] as char);
        out.push(B64[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// Inverse of [`b64_encode`]. Ignores whitespace and `=` padding; returns
/// `None` on any non-base64 byte. Accepts both padded and unpadded input.
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    }
    let bytes: Vec<u8> = s.bytes().filter(|&b| !b.is_ascii_whitespace() && b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        if chunk.len() == 1 {
            return None; // a lone trailing sextet can't be a byte
        }
        let mut n = 0u32;
        for &c in chunk {
            n = (n << 6) | val(c)?;
        }
        let bits = chunk.len() * 6;
        n <<= 24 - bits;
        out.push((n >> 16 & 0xFF) as u8);
        if bits >= 16 {
            out.push((n >> 8 & 0xFF) as u8);
        }
        if bits >= 24 {
            out.push((n & 0xFF) as u8);
        }
    }
    Some(out)
}

/// Collapses runs of whitespace to a single space, up to `cap` characters.
fn collapse_whitespace(chars: impl Iterator<Item = char>, cap: usize) -> String {
    let mut out = String::new();
    let mut last_space = false;
    for c in chars {
        if out.len() >= cap {
            break;
        }
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

impl Clip {
    /// Single-line preview: first non-empty line, whitespace collapsed.
    pub fn preview(&self) -> String {
        let line = self
            .text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        collapse_whitespace(line.trim().chars().take(120), 120)
    }

    /// One-line view of the **whole** clip for the panel list: every line is
    /// joined and all whitespace runs (newlines included) collapse to single
    /// spaces, so a multi-line clip shows content past its first line — you
    /// see more of what a clip actually holds. Capped generously; the row
    /// truncates it to the available width.
    pub fn flattened(&self) -> String {
        collapse_whitespace(self.text.trim().chars(), 200)
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
    /// last. Session-only: lets the panel undo an accidental delete. A bounded
    /// FIFO ring — oldest evicted from the front — so a `VecDeque` keeps both
    /// ends O(1).
    undo: VecDeque<Vec<Clip>>,
    /// Monotonic mutation counter; bumps whenever the list could look
    /// different, so the panel's cached filtered view knows to recompute.
    version: u64,
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

// Search-ranking weights (see [`match_score`] / [`ClipStore::filtered_indices`]).
const PREFIX_BONUS: u32 = 2000; // match at the very start of the field
const WORD_BONUS: u32 = 1000; // match right after a non-alphanumeric char
const EARLY_SPAN: u32 = 500; // earlier matches score up to this much more
const PHRASE_BONUS: i64 = 4000; // the whole multi-token query appears contiguously

/// Allocation-free relevance score of `needle` within `hay` (higher is more
/// relevant), or `None` when `needle` does not occur. Like [`contains_ci`] it
/// folds char-by-char without allocating, but also reports *where* the first
/// match lands so the panel can rank: a prefix beats a word-start beats a
/// mid-word hit, and earlier positions beat later ones.
fn match_score(hay: &str, needle: &str) -> Option<u32> {
    if needle.is_empty() {
        return Some(0);
    }
    let mut prev: Option<char> = None;
    let mut start = hay.char_indices();
    let mut pos: u32 = 0;
    loop {
        let mut h = fold(start.as_str());
        let mut n = fold(needle);
        let matched = loop {
            match (n.next(), h.next()) {
                (None, _) => break true,
                (Some(nc), Some(hc)) if nc == hc => continue,
                _ => break false,
            }
        };
        if matched {
            let boundary = match prev {
                None => PREFIX_BONUS,
                Some(p) if !p.is_alphanumeric() => WORD_BONUS,
                _ => 0,
            };
            return Some(boundary + EARLY_SPAN.saturating_sub(pos));
        }
        match start.next() {
            Some((_, c)) => {
                prev = Some(c);
                pos += 1;
            }
            None => return None,
        }
    }
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
            undo: VecDeque::new(),
            version: 0,
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

    /// Records a plain copy event (no rich formats). Returns `true` when it was
    /// accepted (new clip or an existing one bumped to the top) so the pet can
    /// react. Thin wrapper over [`ClipStore::add_copy_rich`].
    pub fn add_copy(&mut self, text: String, source: Option<String>) -> bool {
        self.add_copy_rich(text, source, None)
    }

    /// Records a copy event, optionally carrying the original rich formats
    /// (ADR-0014). Oversized or empty `formats` are dropped (the plain clip is
    /// still kept); on a duplicate bump, `formats` is refreshed like `source`.
    pub fn add_copy_rich(
        &mut self,
        text: String,
        source: Option<String>,
        formats: Option<RichFormats>,
    ) -> bool {
        if text.trim().is_empty() || text.len() > MAX_TEXT {
            return false;
        }
        let formats = formats.filter(|f| !f.is_empty() && f.byte_len() <= MAX_RICH);
        let ts = now_ts();
        if let Some(i) = self.items.iter().position(|c| c.text == text) {
            // same text copied again: bump to top, refresh meta
            let mut clip = self.items.remove(i);
            clip.ts = ts;
            if source.is_some() {
                clip.source = source;
            }
            if formats.is_some() {
                clip.formats = formats;
            }
            self.items.insert(0, clip);
        } else {
            let clip = Clip {
                id: self.next_id,
                text,
                source,
                pinned: false,
                ts,
                formats,
            };
            self.next_id += 1;
            self.items.insert(0, clip);
            self.evict();
        }
        self.touch();
        true
    }

    fn evict(&mut self) {
        let unpinned: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.pinned)
            .map(|(i, _)| i)
            .collect();
        let excess = unpinned.len().saturating_sub(MAX_HISTORY);
        // Remove oldest (highest indices in newest-first order) first so indices stay valid.
        for &i in unpinned.iter().rev().take(excess) {
            self.items.remove(i);
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
            self.undo.pop_front();
        }
        self.undo.push_back(batch);
    }

    /// Restores the most recent delete/clear operation. Returns the id of one
    /// restored clip (for the panel to re-select), or None if nothing to undo.
    pub fn undo_delete(&mut self) -> Option<u64> {
        let batch = self.undo.pop_back()?;
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
    ///
    /// The query is split on whitespace into tokens; a clip is shown only when
    /// **every** token occurs in its text or source (AND), and the rows are
    /// ranked by relevance (prefix/word-start and earlier hits first, with a
    /// bonus when the whole query appears contiguously). Pinned clips always
    /// come first; ties keep newest-first.
    pub fn filtered_indices(&self, query: &str, source: Option<&str>) -> Vec<usize> {
        let q = query.trim();
        let tokens: Vec<&str> = q.split_whitespace().collect();

        // Source-filter gate + all-tokens-match gate, returning a relevance
        // score (higher is better) or `None` when the clip is filtered out.
        let score = |c: &Clip| -> Option<i64> {
            if let Some(want) = source {
                if !c.source.as_deref().is_some_and(|s| eq_ci(s, want)) {
                    return None;
                }
            }
            if tokens.is_empty() {
                return Some(0);
            }
            let mut total: i64 = 0;
            for tok in &tokens {
                // each token may land in the text or the source app name
                let best =
                    match_score(&c.text, tok).max(c.source.as_deref().and_then(|s| match_score(s, tok)));
                match best {
                    Some(s) => total += i64::from(s),
                    None => return None, // a token matched nowhere => hide the clip
                }
            }
            if tokens.len() > 1 && contains_ci(&c.text, q) {
                total += PHRASE_BONUS;
            }
            Some(total)
        };

        // Ranked indices within one pin-group. `sort_by` is stable, so equal
        // scores preserve the store's newest-first order.
        let rank = |want_pinned: bool| -> Vec<usize> {
            let mut v: Vec<(usize, i64)> = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, c)| c.pinned == want_pinned)
                .filter_map(|(i, c)| score(c).map(|s| (i, s)))
                .collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            v.into_iter().map(|(i, _)| i).collect()
        };

        let mut out = rank(true);
        out.extend(rank(false));
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
        let mut seen = std::collections::HashSet::new();
        let mut out: Vec<&str> = Vec::new();
        for c in &self.items {
            if let Some(s) = c.source.as_deref() {
                if seen.insert(s.to_lowercase()) {
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
    fn search_multi_token_requires_all_tokens() {
        let mut s = ClipStore::default();
        s.add_copy("git clone https://example.com".into(), None);
        s.add_copy("git status".into(), None);
        // both tokens occur only in the first clip
        let v = s.visible("git clone");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].text, "git clone https://example.com");
        // a token that matches nothing hides everything
        assert_eq!(s.visible("git zzz").len(), 0);
    }

    #[test]
    fn search_tokens_span_text_and_source() {
        let mut s = ClipStore::default();
        s.add_copy("clone the repo".into(), Some("Terminal".into()));
        // "terminal" matches the source, "clone" matches the text
        assert_eq!(s.visible("terminal clone").len(), 1);
        assert_eq!(s.visible("terminal missing").len(), 0);
    }

    #[test]
    fn search_multi_token_korean() {
        let mut s = ClipStore::default();
        s.add_copy("안녕 세계".into(), None);
        s.add_copy("안녕하세요".into(), None);
        // "세계" only occurs in the first clip; "안녕" in both
        assert_eq!(s.visible("안녕 세계").len(), 1);
        assert_eq!(s.visible("안녕").len(), 2);
    }

    #[test]
    fn search_ranks_word_start_above_mid_word() {
        let mut s = ClipStore::default();
        s.add_copy("cat food".into(), None); // oldest: prefix match for "cat"
        s.add_copy("scattered notes".into(), None); // newest: mid-word "cat"
        let v = s.visible("cat");
        assert_eq!(v.len(), 2);
        // the prefix hit ranks first despite being older
        assert_eq!(v[0].text, "cat food");
    }

    #[test]
    fn search_ranks_phrase_above_scattered() {
        let mut s = ClipStore::default();
        s.add_copy("git clone the thing".into(), None); // contiguous phrase
        s.add_copy("clone it from git".into(), None); // newest: scattered tokens
        let v = s.visible("git clone");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].text, "git clone the thing");
    }

    #[test]
    fn search_keeps_pinned_first_despite_score() {
        let mut s = ClipStore::default();
        s.add_copy("cat food".into(), None); // strong prefix match, unpinned
        s.add_copy("a scattered cat".into(), None); // weaker match, pinned
        let id = s
            .visible("")
            .iter()
            .find(|c| c.text == "a scattered cat")
            .unwrap()
            .id;
        s.toggle_pin(id);
        let v = s.visible("cat");
        assert_eq!(v.len(), 2);
        assert!(v[0].pinned);
        assert_eq!(v[0].text, "a scattered cat");
    }

    #[test]
    fn match_score_rewards_prefix_and_word_start() {
        let prefix = match_score("cat food", "cat").unwrap();
        let word = match_score("a cat", "cat").unwrap();
        let mid = match_score("scatter", "cat").unwrap();
        assert!(prefix > word && word > mid);
        assert_eq!(match_score("nope", "cat"), None);
        assert_eq!(match_score("anything", ""), Some(0));
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
            formats: None,
        };
        assert_eq!(c.preview(), "fn main() {");
        // flattened folds *every* line in (newlines -> single spaces), so the
        // row shows more than just the first line
        assert_eq!(c.flattened(), "fn main() { body }");
    }

    #[test]
    fn base64_round_trips_including_padding_and_binary() {
        for case in [
            &b""[..],
            &b"f"[..],
            &b"fo"[..],
            &b"foo"[..],
            &b"foob"[..],
            &b"fooba"[..],
            &b"foobar"[..],
            &[0u8, 1, 2, 253, 254, 255][..],
        ] {
            assert_eq!(b64_decode(&b64_encode(case)).as_deref(), Some(case));
        }
        // tolerant of whitespace, strict on junk
        assert_eq!(b64_decode("Zm9v\nYmFy"), Some(b"foobar".to_vec()));
        assert_eq!(b64_decode("not base64!"), None);
    }

    #[test]
    fn rich_formats_store_refresh_and_cap() {
        let mut s = ClipStore::default();
        let fmt = RichFormats { html: Some("<b>hi</b>".into()), rtf_b64: None };
        assert!(s.add_copy_rich("hi".into(), None, Some(fmt.clone())));
        assert_eq!(s.get(s.visible("")[0].id).unwrap().formats, Some(fmt));
        // a bumping plain copy keeps the existing formats; a new one refreshes
        let id = s.visible("")[0].id;
        assert!(s.add_copy("hi".into(), None));
        assert!(s.get(id).unwrap().formats.is_some(), "plain re-copy keeps formats");
        let fmt2 = RichFormats { html: None, rtf_b64: Some(b64_encode(b"{\\rtf1}")) };
        assert!(s.add_copy_rich("hi".into(), None, Some(fmt2.clone())));
        assert_eq!(s.get(id).unwrap().formats, Some(fmt2), "new formats refresh on bump");
        // oversized formats are dropped, the plain clip is still stored
        let huge = RichFormats { html: Some("x".repeat(MAX_RICH + 1)), rtf_b64: None };
        assert!(s.add_copy_rich("plain".into(), None, Some(huge)));
        assert!(s.get(s.visible("plain")[0].id).unwrap().formats.is_none());
    }

    #[test]
    fn formats_are_omitted_for_plain_clips_and_back_compat_loads() {
        // a plain clip serializes without a `formats` key (skip_serializing_if)
        let s = store_with(&["plain"]);
        let json = serde_json::to_string(&ClipsOut { clips: &s.items }).unwrap();
        assert!(!json.contains("formats"), "plain clips omit the key: {json}");
        // old clips.json with no `formats` field loads as None
        let legacy = r#"{"clips":[{"id":1,"text":"old","source":null,"pinned":false,"ts":0}]}"#;
        let back: ClipsFile = serde_json::from_str(legacy).unwrap();
        let s2 = ClipStore::from_items(back.clips);
        assert_eq!(s2.len(), 1);
        assert!(s2.get(1).unwrap().formats.is_none());
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
