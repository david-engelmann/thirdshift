//! The stocked queue, its workers, and its exclusive claims with leases.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::clock::Clock;
use crate::pyjson::Value;

/// The ways a queue operation can be refused, named after the
/// exceptions the Python implementation raised. Every variant
/// prints as the message it carries.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum QueueError {
    /// A worker asked for work and the queue held none. A recorded defect.
    #[error("{0}")]
    Stockout(String),
    /// A second claim was attempted on an item under a live claim, or
    /// a completion arrived from the wrong worker or after the lease.
    #[error("{0}")]
    ClaimConflict(String),
    /// An item identifier was enqueued twice.
    #[error("{0}")]
    DuplicateItem(String),
    /// A lease of zero or negative seconds, refused before any state
    /// changes. Carries the full error message.
    #[error("{0}")]
    InvalidLease(String),
}

/// A validation failure in the runner, carrying its message. Mirrors
/// the ValueError the Python runner raises for a bad window and for
/// a non-empty backlog with no workers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct ValueError(pub String);

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub item_id: String,
    pub title: String,
    pub kind: String,
    pub question: String,
}

impl Item {
    pub fn new(item_id: &str, title: &str) -> Item {
        Item {
            item_id: item_id.to_string(),
            title: title.to_string(),
            kind: "work".to_string(),
            question: String::new(),
        }
    }

    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("id".to_string(), Value::Str(self.item_id.clone())),
            ("title".to_string(), Value::Str(self.title.clone())),
            ("kind".to_string(), Value::Str(self.kind.clone())),
            ("question".to_string(), Value::Str(self.question.clone())),
        ])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    pub item_id: String,
    pub worker_id: String,
    pub expires_at: f64,
}

/// The shared record a claim observer holds. A test keeps one end
/// of the cell and reads back every claim the queue made, in the
/// order the queue made them.
pub type ClaimObserver = Rc<RefCell<Vec<(String, String)>>>;

/// A stocked queue. The clock is required and always injected by the
/// caller, so no lease ever reads the wall clock by accident. There
/// is no way to build a queue without one.
pub struct WorkQueue {
    pub clock: Rc<dyn Clock>,
    pub workers: Vec<String>,
    pub items: HashMap<String, Item>,
    pub order: Vec<String>,
    pub claims: HashMap<String, Claim>,
    pub done: HashMap<String, String>,
    pub stockouts: u64,
    /// An observer hook the tests use to remember every claim, the
    /// same evidence a subclass that records claim calls leaves in
    /// the Python suite. Nothing in the library reads it.
    pub claim_observer: Option<ClaimObserver>,
}

impl WorkQueue {
    pub fn new(clock: Rc<dyn Clock>) -> WorkQueue {
        WorkQueue {
            clock,
            workers: Vec::new(),
            items: HashMap::new(),
            order: Vec::new(),
            claims: HashMap::new(),
            done: HashMap::new(),
            stockouts: 0,
            claim_observer: None,
        }
    }

    pub fn register_worker(&mut self, worker_id: &str) -> String {
        if !self.workers.iter().any(|w| w == worker_id) {
            self.workers.push(worker_id.to_string());
        }
        worker_id.to_string()
    }

    pub fn enqueue(&mut self, item: Item) -> Result<Item, QueueError> {
        if self.items.contains_key(&item.item_id) {
            return Err(QueueError::DuplicateItem(item.item_id.clone()));
        }
        self.order.push(item.item_id.clone());
        self.items.insert(item.item_id.clone(), item.clone());
        Ok(item)
    }

    /// A pure read. True while a claim exists and its lease is
    /// unexpired. This query never deletes anything. Expired claims
    /// are cleared only by prune_expired_claim, which only mutating
    /// operations may call.
    fn claim_is_live(&self, item_id: &str) -> bool {
        match self.claims.get(item_id) {
            Some(claim) => self.clock.now() < claim.expires_at,
            None => false,
        }
    }

    fn prune_expired_claim(&mut self, item_id: &str) {
        let expired = match self.claims.get(item_id) {
            Some(claim) => self.clock.now() >= claim.expires_at,
            None => false,
        };
        if expired {
            self.claims.remove(item_id);
        }
    }

    pub fn pending_ids(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|item_id| !self.done.contains_key(*item_id) && !self.claim_is_live(item_id))
            .cloned()
            .collect()
    }

    pub fn stock_report(&self) -> Value {
        let stocked = self.pending_ids().len();
        let workers = self.workers.len();
        let deficit = workers.saturating_sub(stocked);
        Value::object(vec![
            ("workers".to_string(), Value::usize(workers)),
            ("stocked".to_string(), Value::usize(stocked)),
            ("deficit".to_string(), Value::usize(deficit)),
            ("stockout".to_string(), Value::Bool(deficit > 0)),
            (
                "stockouts_recorded".to_string(),
                Value::uint(self.stockouts),
            ),
        ])
    }

    fn check_lease(lease_seconds: f64) -> Result<(), QueueError> {
        if lease_seconds <= 0.0 {
            return Err(QueueError::InvalidLease(format!(
                "lease_seconds must be positive, got {}",
                crate::pyjson::py_float_repr(lease_seconds)
            )));
        }
        Ok(())
    }

    pub fn claim(
        &mut self,
        item_id: &str,
        worker_id: &str,
        lease_seconds: f64,
    ) -> Result<Claim, QueueError> {
        Self::check_lease(lease_seconds)?;
        if !self.items.contains_key(item_id) || self.done.contains_key(item_id) {
            self.stockouts += 1;
            return Err(QueueError::Stockout(item_id.to_string()));
        }
        self.prune_expired_claim(item_id);
        if self.claims.contains_key(item_id) {
            return Err(QueueError::ClaimConflict(item_id.to_string()));
        }
        let claim = Claim {
            item_id: item_id.to_string(),
            worker_id: worker_id.to_string(),
            expires_at: self.clock.now() + lease_seconds,
        };
        self.claims.insert(item_id.to_string(), claim.clone());
        if let Some(observer) = &self.claim_observer {
            observer
                .borrow_mut()
                .push((item_id.to_string(), worker_id.to_string()));
        }
        Ok(claim)
    }

    pub fn claim_next(&mut self, worker_id: &str, lease_seconds: f64) -> Result<Claim, QueueError> {
        Self::check_lease(lease_seconds)?;
        let pending = self.pending_ids();
        if pending.is_empty() {
            self.stockouts += 1;
            return Err(QueueError::Stockout(worker_id.to_string()));
        }
        self.claim(&pending[0], worker_id, lease_seconds)
    }

    pub fn release(&mut self, item_id: &str, worker_id: &str) -> bool {
        self.prune_expired_claim(item_id);
        let held = match self.claims.get(item_id) {
            Some(claim) => claim.worker_id == worker_id,
            None => false,
        };
        if held {
            self.claims.remove(item_id);
            return true;
        }
        false
    }

    pub fn complete(
        &mut self,
        item_id: &str,
        worker_id: &str,
        outcome: &str,
    ) -> Result<String, QueueError> {
        let claim = match self.claims.get(item_id) {
            Some(claim) => claim.clone(),
            None => return Err(QueueError::ClaimConflict(item_id.to_string())),
        };
        if claim.worker_id != worker_id {
            return Err(QueueError::ClaimConflict(item_id.to_string()));
        }
        if self.clock.now() >= claim.expires_at {
            // The lease is gone, so the completion is refused and
            // nothing changes. The expired claim stays on record
            // until a mutating operation clears it.
            return Err(QueueError::ClaimConflict(item_id.to_string()));
        }
        self.claims.remove(item_id);
        self.done.insert(item_id.to_string(), outcome.to_string());
        Ok(outcome.to_string())
    }
}
