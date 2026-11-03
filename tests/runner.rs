//! The runner corpus, TS-5, TS-6, TS-7, TS-15 to TS-20.

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use common::{field, list, temp_dir, value};
use thirdshift::pyjson::Value;
use thirdshift::runner::RunError;
use thirdshift::{
    load_backlog, run_backlog, FakeClock, Item, RunOptions, WorkQueue,
};

fn item(item_id: &str, title: &str, kind: &str, question: &str) -> Value {
    let mut entry = Item::new(item_id, title);
    entry.kind = kind.to_string();
    entry.question = question.to_string();
    entry.to_value()
}

fn backlog() -> Vec<Value> {
    vec![
        item("task-1", "alpha", "work", ""),
        item("task-2", "broken", "fail", ""),
        item("task-3", "gate", "decision", "Ship the synthetic gate"),
        item("task-4", "omega", "work", ""),
    ]
}

fn run() -> Value {
    let options = RunOptions {
        clock: Some(Rc::new(FakeClock::new(0.0))),
        ..RunOptions::default()
    };
    run_backlog(&backlog(), options).unwrap()
}

fn run_entries(entries: &[Value]) -> Value {
    let options = RunOptions {
        clock: Some(Rc::new(FakeClock::new(0.0))),
        ..RunOptions::default()
    };
    run_backlog(entries, options).unwrap()
}

fn by_id(record: &Value) -> HashMap<String, Value> {
    list(record, "ran")
        .iter()
        .map(|entry| {
            (
                entry.get("id").unwrap().as_str().unwrap().to_string(),
                entry.clone(),
            )
        })
        .collect()
}

fn text(entry: &Value, name: &str) -> String {
    entry
        .get(name)
        .and_then(|found| found.as_str())
        .unwrap_or("")
        .to_string()
}

#[test]
fn every_backlog_item_appears_exactly_once_in_the_record() {
    let record = run();
    let mut ids: Vec<String> = list(&record, "ran")
        .iter()
        .map(|entry| text(entry, "id"))
        .collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids, vec!["task-1", "task-2", "task-3", "task-4"]);
    assert_eq!(total, ids.len(), "an item was recorded twice");
}

#[test]
fn counts_add_up_to_the_backlog_size() {
    let record = run();
    let counts = field(&record, "counts");
    assert_eq!(field(counts, "backlog"), &Value::uint(4));
    assert_eq!(field(counts, "recorded"), &Value::uint(4));
    let finished = field(counts, "finished").clone();
    let unfinished = field(counts, "unfinished").clone();
    let total = match (&finished, &unfinished) {
        (Value::Int(a), Value::Int(b)) => a.parse::<i64>().unwrap() + b.parse::<i64>().unwrap(),
        other => panic!("counts are not integers, {:?}", other),
    };
    assert_eq!(total, 4);
}

#[test]
fn unfinished_items_carry_a_reason_and_finished_items_carry_output() {
    let record = run();
    let entries = by_id(&record);
    assert_eq!(text(&entries["task-2"], "status"), "unfinished");
    assert!(!text(&entries["task-2"], "reason").is_empty());
    assert_eq!(text(&entries["task-1"], "status"), "finished");
    assert_eq!(text(&entries["task-1"], "output"), "completed task-1");
    let produced: Vec<String> = list(&record, "produced")
        .iter()
        .map(|output| output.as_str().unwrap().to_string())
        .collect();
    assert!(!produced.contains(&"completed task-2".to_string()));
}

#[test]
fn every_entry_names_a_registered_worker() {
    let record = run();
    for entry in list(&record, "ran") {
        let worker = text(entry, "worker");
        assert!(!worker.is_empty());
        assert!(
            ["worker-1", "worker-2"].contains(&worker.as_str()),
            "worker {}",
            worker
        );
    }
}

#[test]
fn decision_task_queues_a_decision_with_both_responses() {
    let record = run();
    let decisions = list(&record, "decisions");
    assert_eq!(decisions.len(), 1);
    let decision = &decisions[0];
    assert_eq!(text(decision, "item"), "task-3");
    assert_eq!(text(decision, "question"), "Ship the synthetic gate");
    assert_eq!(
        field(decision, "options"),
        &value(r#"["approve", "reject"]"#)
    );
}

#[test]
fn work_tasks_queue_no_decisions() {
    let entries = vec![item("task-9", "plain", "work", "")];
    let record = run_entries(&entries);
    assert!(list(&record, "decisions").is_empty());
    assert_eq!(field(&record, "produced"), &value(r#"["completed task-9"]"#));
}

#[test]
fn runner_claims_every_item_it_records_through_the_queue() {
    // The claim observer remembers every claim, so a runner that
    // fabricated worker names without claiming leaves an empty log.
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut queue = WorkQueue::new(Rc::new(FakeClock::new(0.0)));
    queue.claim_observer = Some(log.clone());
    let options = RunOptions {
        clock: Some(queue.clock.clone()),
        queue: Some(&mut queue),
        ..RunOptions::default()
    };
    let record = run_backlog(&backlog(), options).unwrap();
    let calls = log.borrow().clone();
    let claimed: HashMap<String, String> = calls.iter().cloned().collect();
    let mut claimed_ids: Vec<&String> = claimed.keys().collect();
    claimed_ids.sort();
    assert_eq!(claimed_ids, ["task-1", "task-2", "task-3", "task-4"]);
    assert_eq!(calls.len(), 4);
    for entry in list(&record, "ran") {
        assert_eq!(claimed[&text(entry, "id")], text(entry, "worker"));
    }
}

#[test]
fn duplicate_item_ids_are_recorded_and_the_run_still_completes() {
    let entries = vec![
        value(r#"{"id": "task-1", "title": "alpha"}"#),
        value(r#"{"id": "task-2", "title": "beta"}"#),
        value(r#"{"id": "task-1", "title": "alpha again"}"#),
    ];
    let record = run_entries(&entries);
    assert_eq!(
        field(&record, "counts"),
        &value(r#"{"backlog": 3, "finished": 2, "recorded": 3, "unfinished": 1}"#)
    );
    let duplicates: Vec<&Value> = list(&record, "ran")
        .iter()
        .filter(|entry| text(entry, "id") == "task-1" && text(entry, "status") == "unfinished")
        .collect();
    assert_eq!(duplicates.len(), 1);
    assert!(text(duplicates[0], "reason").contains("duplicate"));
    let produced: Vec<String> = list(&record, "produced")
        .iter()
        .map(|output| output.as_str().unwrap().to_string())
        .collect();
    assert!(produced.contains(&"completed task-1".to_string()));
}

#[test]
fn duplicate_item_objects_are_recorded_the_same_way() {
    let entries = vec![
        item("task-1", "alpha", "work", ""),
        item("task-1", "alpha again", "work", ""),
    ];
    let record = run_entries(&entries);
    assert_eq!(
        field(&record, "counts"),
        &value(r#"{"backlog": 2, "finished": 1, "recorded": 2, "unfinished": 1}"#)
    );
}

#[test]
fn a_question_without_the_decision_kind_is_recorded_not_run_as_work() {
    let entries = vec![value(
        r#"{"id": "task-1", "title": "Approve the gate", "question": "Ship the synthetic gate"}"#,
    )];
    let record = run_entries(&entries);
    assert!(list(&record, "decisions").is_empty());
    assert!(list(&record, "produced").is_empty());
    let ran = list(&record, "ran");
    assert_eq!(text(&ran[0], "status"), "unfinished");
    assert!(text(&ran[0], "reason").contains("malformed decision"));
}

#[test]
fn a_work_kind_with_a_question_is_also_a_malformed_decision() {
    let entries = vec![value(
        r#"{"id": "task-1", "title": "Approve the gate", "kind": "work", "question": "Ship it"}"#,
    )];
    let record = run_entries(&entries);
    let ran = list(&record, "ran");
    assert_eq!(text(&ran[0], "status"), "unfinished");
    assert!(text(&ran[0], "reason").contains("malformed decision"));
}

#[test]
fn a_decision_task_without_a_question_falls_back_to_its_title() {
    let entries = vec![value(
        r#"{"id": "task-1", "title": "Approve the synthetic gate", "kind": "decision"}"#,
    )];
    let record = run_entries(&entries);
    let decisions = list(&record, "decisions");
    assert_eq!(decisions.len(), 1);
    assert_eq!(text(&decisions[0], "question"), "Approve the synthetic gate");
}

#[test]
fn malformed_entries_are_recorded_and_the_rest_still_run() {
    let entries = vec![
        value(r#"{"title": "no id on this one"}"#),
        Value::Str("not a task at all".to_string()),
        Value::Null,
        value(r#"{"id": "task-1", "title": "alpha"}"#),
    ];
    let record = run_entries(&entries);
    assert_eq!(
        field(&record, "counts"),
        &value(r#"{"backlog": 4, "finished": 1, "recorded": 4, "unfinished": 3}"#)
    );
    let bad: Vec<&Value> = list(&record, "ran")
        .iter()
        .filter(|entry| text(entry, "status") == "unfinished")
        .collect();
    assert_eq!(bad.len(), 3);
    assert!(bad.iter().all(|entry| !text(entry, "reason").is_empty()));
}

#[test]
fn an_entry_with_only_an_id_still_runs() {
    let entries = vec![value(r#"{"id": "task-7"}"#)];
    let record = run_entries(&entries);
    assert_eq!(field(field(&record, "counts"), "finished"), &Value::uint(1));
    assert_eq!(field(&record, "produced"), &value(r#"["completed task-7"]"#));
}

#[test]
fn no_workers_for_a_non_empty_backlog_is_a_clear_error() {
    let entries = vec![item("task-1", "alpha", "work", "")];
    let options = RunOptions {
        worker_ids: Vec::new(),
        clock: Some(Rc::new(FakeClock::new(0.0))),
        ..RunOptions::default()
    };
    let error = run_backlog(&entries, options).unwrap_err();
    assert!(matches!(error, RunError::Value(_)));
    let options = RunOptions {
        worker_ids: Vec::new(),
        clock: Some(Rc::new(FakeClock::new(0.0))),
        ..RunOptions::default()
    };
    let record = run_backlog(&[], options).unwrap();
    assert_eq!(
        field(&record, "counts"),
        &value(r#"{"backlog": 0, "finished": 0, "recorded": 0, "unfinished": 0}"#)
    );
}

#[test]
fn load_backlog_rejects_invalid_json() {
    let dir = temp_dir("load-json");
    let path = dir.join("backlog.json");
    std::fs::write(&path, "{not json").unwrap();
    assert!(load_backlog(&path).is_err());
}

#[test]
fn load_backlog_rejects_an_object_without_a_tasks_list() {
    let dir = temp_dir("load-shape");
    let path = dir.join("backlog.json");
    std::fs::write(&path, r#"{"items": []}"#).unwrap();
    assert!(load_backlog(&path).is_err());
}

#[test]
fn wrongly_typed_fields_are_recorded_not_crashing() {
    let entries = vec![
        value(r#"{"id": 5, "title": "numeric id"}"#),
        value(r#"{"id": ["a"], "title": "list id"}"#),
        value(r#"{"id": "t-title", "title": 3}"#),
        value(r#"{"id": "t-kind", "kind": 7}"#),
        value(r#"{"id": "t-question", "kind": "decision", "question": 9}"#),
        value(r#"{"id": "", "title": "empty id"}"#),
        value(r#"{"id": "good", "title": "fine"}"#),
    ];
    let record = run_entries(&entries);
    let counts = field(&record, "counts");
    assert_eq!(field(counts, "backlog"), &Value::uint(7));
    assert_eq!(field(counts, "recorded"), &Value::uint(7));
    assert_eq!(field(counts, "finished"), &Value::uint(1));
    let reasons: Vec<String> = list(&record, "ran")
        .iter()
        .map(|entry| text(entry, "reason"))
        .collect();
    for needle in [
        "id is not a non-empty string",
        "title is not a string",
        "kind is not a string",
        "question is not a string",
    ] {
        assert!(
            reasons.iter().any(|reason| reason.contains(needle)),
            "no recorded reason names {}, reasons {:?}",
            needle,
            reasons
        );
    }
}
