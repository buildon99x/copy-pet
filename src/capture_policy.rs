//! Portable reads can outlive the policy that authorized them. Each pending
//! source has a revocable permit shared by the watcher and UI delivery queue.
use crate::clipboard::eq_ci;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Weak,
};

#[derive(Clone)]
pub(crate) struct SourcePolicy(Arc<Mutex<Rules>>);

struct Rules {
    excluded: Vec<String>,
    pending: Vec<(String, Weak<AtomicBool>)>,
}

#[derive(Clone)]
pub(crate) struct Permit(Arc<AtomicBool>);

impl Permit {
    pub fn valid(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl SourcePolicy {
    pub fn new(excluded: &[String]) -> Self {
        Self(Arc::new(Mutex::new(Rules {
            excluded: excluded.to_vec(),
            pending: Vec::new(),
        })))
    }

    pub fn permit(&self, source: Option<&str>) -> Option<Permit> {
        let mut rules = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let source = source.unwrap_or("");
        if rules.excluded.iter().any(|s| eq_ci(s, source)) {
            return None;
        }
        rules.pending.retain(|(_, weak)| weak.strong_count() > 0);
        if let Some(active) = rules
            .pending
            .iter()
            .find(|(s, _)| eq_ci(s, source))
            .and_then(|(_, w)| w.upgrade())
        {
            return Some(Permit(active));
        }
        let active = Arc::new(AtomicBool::new(true));
        rules
            .pending
            .push((source.to_owned(), Arc::downgrade(&active)));
        Some(Permit(active))
    }

    pub fn update(&self, excluded: &[String]) {
        let mut rules = self.0.lock().unwrap_or_else(|e| e.into_inner());
        rules.pending.retain(|(name, weak)| {
            let Some(permit) = weak.upgrade() else {
                return false;
            };
            if excluded.iter().any(|s| eq_ci(name, s)) {
                permit.store(false, Ordering::Release);
                return false;
            }
            true
        });
        rules.excluded = excluded.to_vec();
    }
}
