//! Session-only clipboard snapshots for ordered paste.

use crate::clipboard::Clip;

/// Keep snapshots when the history capacity evicts a row. Completed rows remain
/// visible for replay until the user clears or edits the queue.
#[derive(Default)]
pub struct PasteQueue {
    entries: Vec<Clip>,
    completed: Vec<u64>,
    revision: u64,
}

impl PasteQueue {
    pub const LIMIT: usize = 16;

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn entries(&self) -> &[Clip] {
        &self.entries
    }

    pub fn toggle(&mut self, clip: &Clip) -> bool {
        if self.entries.iter().any(|entry| entry.id == clip.id) {
            self.retain(|entry| entry.id != clip.id);
        } else if self.entries.len() < Self::LIMIT {
            self.entries.push(clip.clone());
            self.revision = self.revision.wrapping_add(1);
        } else {
            return false;
        }
        true
    }

    pub fn retain(&mut self, keep: impl FnMut(&Clip) -> bool) {
        let before = self.entries.len();
        self.entries.retain(keep);
        if before != self.entries.len() {
            self.revision = self.revision.wrapping_add(1);
        }
        self.completed
            .retain(|id| self.entries.iter().any(|entry| entry.id == *id));
    }

    pub fn update_pin(&mut self, id: u64, pinned: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.pinned = pinned;
        }
    }

    pub fn shift(&mut self, id: u64, delta: isize) {
        if let Some(index) = self.entries.iter().position(|entry| entry.id == id) {
            let target = index
                .saturating_add_signed(delta)
                .min(self.entries.len() - 1);
            if index != target {
                self.entries.swap(index, target);
                self.revision = self.revision.wrapping_add(1);
            }
        }
    }

    pub fn clear(&mut self) {
        self.retain(|_| false);
    }

    pub fn next(&self) -> Option<&Clip> {
        self.entries
            .iter()
            .find(|entry| !self.completed.contains(&entry.id))
    }

    pub fn previous(&self) -> Option<&Clip> {
        let id = self.completed.last()?;
        self.entries.iter().find(|entry| entry.id == *id)
    }

    pub fn acknowledge(&mut self, id: u64, success: bool) {
        if success && self.next().is_some_and(|entry| entry.id == id) {
            self.completed.push(id);
        }
    }

    pub fn combined(&self) -> Option<String> {
        if self.entries.is_empty() || self.entries.iter().any(|entry| entry.image.is_some()) {
            return None;
        }
        Some(
            self.entries
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(id: u64, text: &str) -> Clip {
        serde_json::from_value(
            serde_json::json!({"id":id,"text":text,"source":"test","ts":1,"pinned":false}),
        )
        .unwrap()
    }

    #[test]
    fn failed_output_retries_same_snapshot_and_replay_preserves_order() {
        let mut q = PasteQueue::default();
        q.toggle(&sample(3, "three"));
        q.toggle(&sample(1, "one"));
        q.shift(1, -1);
        assert_eq!(q.combined().unwrap(), "one\nthree");
        q.acknowledge(1, false);
        assert_eq!(q.next().unwrap().id, 1);
        q.acknowledge(1, true);
        assert_eq!(q.next().unwrap().id, 3);
        assert_eq!(q.previous().unwrap().id, 1);
        q.toggle(&sample(3, "three"));
        assert!(q.next().is_none());
        q.clear();
        assert!(q.previous().is_none());
    }
}
