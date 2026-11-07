//! The stock corpus, TS-1, TS-2, TS-13, TS-14.

use std::rc::Rc;

use thirdshift::pyjson::Value;
use thirdshift::queue::QueueError;
use thirdshift::{Clock, FakeClock, Item, WorkQueue};

fn build_queue(item_ids: &[&str], worker_ids: &[&str]) -> WorkQueue {
    let mut queue = WorkQueue::new(Rc::new(FakeClock::new(100.0)));
    for worker_id in worker_ids {
        queue.register_worker(worker_id);
    }
    for item_id in item_ids {
        queue.enqueue(Item::new(item_id, item_id)).unwrap();
    }
    queue
}

fn number(report: &Value, name: &str) -> i64 {
    match report.get(name) {
        Some(Value::Int(text)) => text.parse().unwrap(),
        other => panic!("report field {} is {:?}", name, other),
    }
}

fn flag(report: &Value, name: &str) -> bool {
    match report.get(name) {
        Some(Value::Bool(value)) => *value,
        other => panic!("report field {} is {:?}", name, other),
    }
}

#[test]
fn stock_report_counts_workers_items_and_deficit() {
    let queue = build_queue(&["task-1", "task-2", "task-3"], &["worker-1", "worker-2"]);
    let report = queue.stock_report();
    assert_eq!(number(&report, "workers"), 2);
    assert_eq!(number(&report, "stocked"), 3);
    assert_eq!(number(&report, "deficit"), 0);
    assert!(!flag(&report, "stockout"));
}

#[test]
fn stock_report_shows_a_deficit_below_the_worker_count() {
    let queue = build_queue(&["task-1"], &["worker-1", "worker-2", "worker-3"]);
    let report = queue.stock_report();
    assert_eq!(number(&report, "stocked"), 1);
    assert_eq!(number(&report, "deficit"), 2);
    assert!(flag(&report, "stockout"));
}

#[test]
fn a_claimed_item_no_longer_counts_as_stocked() {
    let mut queue = build_queue(&["task-1", "task-2"], &["worker-1", "worker-2"]);
    queue.claim_next("worker-1", 60.0).unwrap();
    let report = queue.stock_report();
    assert_eq!(number(&report, "stocked"), 1);
    assert_eq!(number(&report, "deficit"), 1);
}

#[test]
fn claim_next_on_an_empty_queue_is_a_recorded_stockout() {
    let mut queue = build_queue(&[], &["worker-1"]);
    let error = queue.claim_next("worker-1", 60.0).unwrap_err();
    assert!(matches!(error, QueueError::Stockout(_)));
    assert_eq!(queue.stockouts, 1);
    assert_eq!(number(&queue.stock_report(), "stockouts_recorded"), 1);
}

#[test]
fn every_stockout_is_recorded() {
    let mut queue = build_queue(&[], &["worker-1"]);
    for _ in 0..3 {
        assert!(queue.claim_next("worker-1", 60.0).is_err());
    }
    assert_eq!(queue.stockouts, 3);
}

#[test]
fn stockouts_from_direct_claims_share_the_same_counter() {
    let mut queue = build_queue(&["task-1"], &["worker-1"]);
    queue.claim("task-1", "worker-1", 30.0).unwrap();
    queue.complete("task-1", "worker-1", "finished").unwrap();
    assert!(matches!(
        queue.claim("task-1", "worker-1", 30.0).unwrap_err(),
        QueueError::Stockout(_)
    ));
    assert!(matches!(
        queue.claim("task-x", "worker-1", 30.0).unwrap_err(),
        QueueError::Stockout(_)
    ));
    assert_eq!(queue.stockouts, 2);
    assert_eq!(number(&queue.stock_report(), "stockouts_recorded"), 2);
}

#[test]
fn reading_the_queue_does_not_prune_expired_claims() {
    let clock = Rc::new(FakeClock::new(100.0));
    let mut queue = WorkQueue::new(clock.clone());
    queue.register_worker("worker-1");
    queue.enqueue(Item::new("task-1", "task-1")).unwrap();
    queue.claim("task-1", "worker-1", 30.0).unwrap();
    clock.advance(60.0);
    let before: Vec<(String, String, f64)> = queue
        .claims
        .iter()
        .map(|(id, claim)| (id.clone(), claim.worker_id.clone(), claim.expires_at))
        .collect();
    // The report and the pending list are pure reads under TS-13.
    // The expired claim still counts as stocked, because pruning
    // belongs to the claim paths alone.
    let report = queue.stock_report();
    let pending = queue.pending_ids();
    let after: Vec<(String, String, f64)> = queue
        .claims
        .iter()
        .map(|(id, claim)| (id.clone(), claim.worker_id.clone(), claim.expires_at))
        .collect();
    assert_eq!(before, after);
    assert_eq!(pending, vec!["task-1".to_string()]);
    assert_eq!(number(&report, "stocked"), 1);
}
