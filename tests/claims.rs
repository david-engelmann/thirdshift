//! The claims corpus, TS-3, TS-11, TS-12, TS-22.
//!
//! One Python pin has no runtime counterpart here. Building a
//! queue without a clock is a Python TypeError, and in Rust it
//! does not compile at all, WorkQueue::new takes the clock as a
//! required argument, so the pin holds structurally under TS-36.

use std::rc::Rc;

use thirdshift::queue::QueueError;
use thirdshift::{Clock, FakeClock, Item, WorkQueue};

fn queue_at(start: f64) -> (WorkQueue, Rc<FakeClock>) {
    let clock = Rc::new(FakeClock::new(start));
    let mut queue = WorkQueue::new(clock.clone());
    queue.register_worker("worker-1");
    queue.register_worker("worker-2");
    (queue, clock)
}

fn enqueue(queue: &mut WorkQueue, item_id: &str) {
    queue
        .enqueue(Item::new(item_id, "synthetic task"))
        .unwrap();
}

#[test]
fn second_claim_conflicts_and_changes_nothing() {
    let (mut queue, _clock) = queue_at(100.0);
    enqueue(&mut queue, "task-1");
    let first = queue.claim("task-1", "worker-1", 30.0).unwrap();
    let error = queue.claim("task-1", "worker-2", 30.0).unwrap_err();
    assert!(matches!(error, QueueError::ClaimConflict(_)));
    let recorded = &queue.claims["task-1"];
    assert_eq!(recorded.worker_id, "worker-1");
    assert_eq!(recorded.expires_at, first.expires_at);
}

#[test]
fn expired_claim_frees_the_item_at_the_exact_boundary() {
    let (mut queue, clock) = queue_at(100.0);
    enqueue(&mut queue, "task-1");
    queue.claim("task-1", "worker-1", 30.0).unwrap();
    clock.advance(29.0);
    assert!(queue.claim("task-1", "worker-2", 30.0).is_err());
    clock.advance(1.0);
    let claim = queue.claim("task-1", "worker-2", 30.0).unwrap();
    assert_eq!(claim.worker_id, "worker-2");
}

#[test]
fn release_by_the_holder_frees_the_item_at_once() {
    let (mut queue, _clock) = queue_at(100.0);
    enqueue(&mut queue, "task-1");
    queue.claim("task-1", "worker-1", 30.0).unwrap();
    assert!(queue.release("task-1", "worker-1"));
    let claim = queue.claim("task-1", "worker-2", 30.0).unwrap();
    assert_eq!(claim.worker_id, "worker-2");
}

#[test]
fn release_by_a_non_holder_frees_nothing() {
    let (mut queue, _clock) = queue_at(100.0);
    enqueue(&mut queue, "task-1");
    queue.claim("task-1", "worker-1", 30.0).unwrap();
    assert!(!queue.release("task-1", "worker-2"));
    assert!(queue.claim("task-1", "worker-2", 30.0).is_err());
}

#[test]
fn completion_requires_the_claiming_worker() {
    let (mut queue, _clock) = queue_at(100.0);
    enqueue(&mut queue, "task-1");
    queue.claim("task-1", "worker-1", 30.0).unwrap();
    assert!(queue.complete("task-1", "worker-2", "finished").is_err());
    queue.complete("task-1", "worker-1", "finished").unwrap();
    assert!(queue.pending_ids().is_empty());
}

#[test]
fn completion_after_expiry_is_refused_and_changes_nothing() {
    let (mut queue, clock) = queue_at(100.0);
    enqueue(&mut queue, "task-1");
    queue.claim("task-1", "worker-1", 30.0).unwrap();
    clock.advance(31.0);
    let error = queue.complete("task-1", "worker-1", "finished").unwrap_err();
    assert!(matches!(error, QueueError::ClaimConflict(_)));
    assert!(queue.done.is_empty());
    assert_eq!(queue.claims["task-1"].worker_id, "worker-1");
    // The item is free for a fresh claim once the dead one is pruned
    // by that claim, and the fresh claim can complete.
    queue.claim("task-1", "worker-2", 30.0).unwrap();
    queue.complete("task-1", "worker-2", "finished").unwrap();
    assert_eq!(queue.done["task-1"], "finished");
}

#[test]
fn completion_at_the_exact_lease_boundary_is_refused() {
    let (mut queue, clock) = queue_at(100.0);
    enqueue(&mut queue, "task-1");
    queue.claim("task-1", "worker-1", 30.0).unwrap();
    clock.advance(30.0);
    assert!(queue.complete("task-1", "worker-1", "finished").is_err());
    assert!(queue.done.is_empty());
}

#[test]
fn non_positive_lease_is_rejected_before_any_state_changes() {
    for lease in [0.0, -50.0] {
        let (mut queue, _clock) = queue_at(100.0);
        enqueue(&mut queue, "task-1");
        let error = queue.claim("task-1", "worker-1", lease).unwrap_err();
        assert!(matches!(error, QueueError::InvalidLease(_)), "lease {}", lease);
        assert!(queue.claims.is_empty());
        let error = queue.claim_next("worker-1", lease).unwrap_err();
        assert!(matches!(error, QueueError::InvalidLease(_)), "lease {}", lease);
    }
}

#[test]
fn claim_next_hands_out_items_in_enqueue_order() {
    let (mut queue, _clock) = queue_at(100.0);
    enqueue(&mut queue, "task-b");
    enqueue(&mut queue, "task-a");
    let claim = queue.claim_next("worker-1", 60.0).unwrap();
    assert_eq!(claim.item_id, "task-b");
}
