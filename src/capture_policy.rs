//! A watcher must apply the policy from the start of its observation, even if
//! a menu changes that policy before source lookup, payload read or UI delivery.
use crate::clipboard::eq_ci;
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone)]
pub(crate) struct SourcePolicy(Arc<Mutex<Rules>>);

struct Rules {
    current: SourceEpoch,
    pending: Vec<Weak<Mutex<Vec<String>>>>,
}

#[derive(Clone)]
pub(crate) struct SourceEpoch(Arc<Mutex<Vec<String>>>);

impl SourceEpoch {
    pub fn allows(&self, source: Option<&str>) -> bool {
        source.is_none_or(|name| {
            !self
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .any(|s| eq_ci(s, name))
        })
    }
}

impl SourcePolicy {
    pub fn new(excluded: &[String]) -> Self {
        Self(Arc::new(Mutex::new(Rules {
            current: SourceEpoch(Arc::new(Mutex::new(excluded.to_vec()))),
            pending: Vec::new(),
        })))
    }

    pub fn snapshot(&self) -> SourceEpoch {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current
            .clone()
    }

    pub fn update(&self, changed: &str, excluded: &[String]) {
        let mut rules = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let previous = Arc::downgrade(&rules.current.0);
        rules.pending.push(previous);
        rules.current = SourceEpoch(Arc::new(Mutex::new(excluded.to_vec())));
        let revoked = excluded.iter().any(|s| eq_ci(s, changed));
        rules.pending.retain(|weak| {
            let Some(epoch) = weak.upgrade() else {
                return false;
            };
            if revoked {
                let mut blocked = epoch.lock().unwrap_or_else(|e| e.into_inner());
                if !blocked.iter().any(|s| eq_ci(s, changed)) {
                    blocked.push(changed.to_owned());
                }
            }
            true
        });
    }
}
