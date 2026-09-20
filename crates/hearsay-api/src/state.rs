//! Shared state: the policy engine and an in-memory decision log.
//!
//! The log is a bounded ring, not the SQLite store `design.md` §7 calls for.
//! That is deliberate for this stage — the store is `hearsay-store`, which
//! does not exist yet. Decisions do not survive a restart, and the API says
//! so in `/readyz`.

use std::collections::VecDeque;
use std::sync::{PoisonError, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use hearsay_policy::{PolicyEngine, Ruleset, RulesetError};

use crate::record::DecisionRecord;

/// How many decisions the ring holds before the oldest is dropped.
pub(crate) const MAX_DECISIONS: usize = 500;

pub(crate) struct AppState {
    pub(crate) engine: PolicyEngine,
    log: RwLock<VecDeque<DecisionRecord>>,
    pub(crate) started_ms: u64,
}

impl AppState {
    pub(crate) fn new(ruleset: Ruleset) -> Result<Self, RulesetError> {
        Ok(Self {
            engine: PolicyEngine::new(ruleset)?,
            log: RwLock::new(VecDeque::with_capacity(MAX_DECISIONS)),
            started_ms: now_ms(),
        })
    }

    /// Append a decision, dropping the oldest once the ring is full.
    ///
    /// The lock is never held across an await — the critical section is a
    /// push and a truncate.
    pub(crate) fn record(&self, decision: DecisionRecord) {
        let mut log = self.log.write().unwrap_or_else(PoisonError::into_inner);
        log.push_front(decision);
        while log.len() > MAX_DECISIONS {
            log.pop_back();
        }
    }

    /// Most recent decisions first.
    ///
    /// `since_ms` returns only decisions newer than that timestamp, which is
    /// how the dashboard polls without refetching what it already has.
    pub(crate) fn recent(&self, since_ms: Option<u64>, limit: usize) -> Vec<DecisionRecord> {
        let log = self.log.read().unwrap_or_else(PoisonError::into_inner);
        log.iter()
            .take_while(|d| since_ms.is_none_or(|since| d.ts_ms > since))
            .take(limit)
            .cloned()
            .collect()
    }

    pub(crate) fn get(&self, decision_id: &str) -> Option<DecisionRecord> {
        let log = self.log.read().unwrap_or_else(PoisonError::into_inner);
        log.iter()
            .find(|d| d.decision_id.to_string() == decision_id)
            .cloned()
    }

    /// Snapshot the whole log for aggregation.
    pub(crate) fn all(&self) -> Vec<DecisionRecord> {
        let log = self.log.read().unwrap_or_else(PoisonError::into_inner);
        log.iter().cloned().collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.log
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// Unix epoch milliseconds.
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
