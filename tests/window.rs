//! The window corpus, TS-29 to TS-33.

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use common::{field, list, root, value};
use thirdshift::pyjson::{self, Value};
use thirdshift::runner::{RunError, WindowValue};
use thirdshift::{
    FakeClock, Item, RunOptions, WorkQueue, apply_resolutions, brief_json, brief_markdown,
    load_backlog, run_backlog,
};

const REASON: &str = "the window closed before this item was started";

fn item(item_id: &str, title: &str, kind: &str, question: &str) -> Value {
    let mut entry = Item::new(item_id, title);
    entry.kind = kind.to_string();
    entry.question = question.to_string();
    entry.to_value()
}

fn backlog() -> Vec<Value> {
    vec![
        item("task-1", "alpha", "work", ""),
        item("task-2", "beta", "work", ""),
        item("task-3", "gamma", "work", ""),
        item("task-4", "gate", "decision", "Ship the synthetic gate"),
        item("task-5", "omega", "work", ""),
    ]
}

fn run_window(entries: &[Value], window: Option<WindowValue>) -> Value {
    run_window_full(entries, window, 1.0, 0.0)
}

fn run_window_full(
    entries: &[Value],
    window: Option<WindowValue>,
    advance_per_item: f64,
    clock_start: f64,
) -> Value {
    let options = RunOptions {
        clock: Some(Rc::new(FakeClock::new(clock_start))),
        advance_per_item,
        window,
        ..RunOptions::default()
    };
    run_backlog(entries, options).unwrap()
}

fn by_id(record: &Value) -> HashMap<String, Value> {
    list(record, "ran")
        .iter()
        .map(|entry| (text(entry, "id"), entry.clone()))
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
fn a_window_that_closes_mid_backlog_leaves_the_exact_tail_unstarted() {
    let record = run_window(&backlog(), Some(WindowValue::from(2i64)));
    let entries = by_id(&record);
    assert_eq!(text(&entries["task-1"], "status"), "finished");
    assert_eq!(text(&entries["task-2"], "status"), "finished");
    for item_id in ["task-3", "task-4", "task-5"] {
        let entry = &entries[item_id];
        assert_eq!(text(entry, "status"), "unfinished", "{}", item_id);
        assert_eq!(text(entry, "reason"), REASON, "{}", item_id);
        assert_eq!(text(entry, "worker"), "", "{}", item_id);
        assert_eq!(text(entry, "output"), "", "{}", item_id);
        assert!(!text(entry, "title").is_empty(), "{}", item_id);
    }
    assert_eq!(
        field(&record, "counts"),
        &value(r#"{"backlog": 5, "finished": 2, "recorded": 5, "unfinished": 3}"#)
    );
    assert_eq!(
        field(&record, "produced"),
        &value(r#"["completed task-1", "completed task-2"]"#)
    );
}

#[test]
fn the_window_reaching_its_end_exactly_starts_no_further_item() {
    let record = run_window(&backlog(), Some(WindowValue::from(1i64)));
    let entries = by_id(&record);
    assert_eq!(text(&entries["task-1"], "status"), "finished");
    assert_eq!(text(&entries["task-2"], "status"), "unfinished");
    assert_eq!(text(&entries["task-2"], "reason"), REASON);
    assert_eq!(field(field(&record, "window"), "started"), &Value::uint(1));
}

#[test]
fn window_closed_items_carry_no_claim_and_no_lease_and_no_stockout() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut queue = WorkQueue::new(Rc::new(FakeClock::new(0.0)));
    queue.claim_observer = Some(log.clone());
    let options = RunOptions {
        clock: Some(queue.clock.clone()),
        queue: Some(&mut queue),
        window: Some(WindowValue::from(2i64)),
        ..RunOptions::default()
    };
    let record = run_backlog(&backlog(), options).unwrap();
    let claimed_ids: Vec<String> = log.borrow().iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(claimed_ids, vec!["task-1", "task-2"]);
    let mut done_ids: Vec<String> = queue.done.keys().cloned().collect();
    done_ids.sort();
    assert_eq!(done_ids, vec!["task-1", "task-2"]);
    assert!(queue.claims.is_empty());
    assert_eq!(queue.stockouts, 0);
    assert_eq!(
        field(&record, "stock"),
        &value(r#"{"stockouts_recorded": 0, "stocked_at_start": 5, "workers": 2}"#)
    );
}

#[test]
fn a_window_closed_decision_item_queues_no_decision() {
    let record = run_window(&backlog(), Some(WindowValue::from(2i64)));
    assert!(list(&record, "decisions").is_empty());
}

#[test]
fn an_item_started_inside_the_window_is_worked_past_its_end() {
    let entries = vec![
        item("task-1", "alpha", "work", ""),
        item("task-2", "beta", "work", ""),
        item("task-3", "gamma", "work", ""),
    ];
    let record = run_window_full(&entries, Some(WindowValue::from(3i64)), 5.0, 0.0);
    let entries_by_id = by_id(&record);
    assert_eq!(text(&entries_by_id["task-1"], "status"), "finished");
    assert_eq!(text(&entries_by_id["task-1"], "output"), "completed task-1");
    assert_eq!(text(&entries_by_id["task-2"], "reason"), REASON);
    assert_eq!(text(&entries_by_id["task-3"], "reason"), REASON);
    assert_eq!(
        field(&record, "window"),
        &value(r#"{"elapsed_seconds": 5.0, "started": 1, "unstarted": 2, "window_seconds": 3.0}"#)
    );
}

#[test]
fn a_zero_window_starts_nothing_and_still_produces_a_brief() {
    let record = run_window(&backlog(), Some(WindowValue::from(0i64)));
    assert_eq!(
        field(&record, "counts"),
        &value(r#"{"backlog": 5, "finished": 0, "recorded": 5, "unfinished": 5}"#)
    );
    assert!(list(&record, "produced").is_empty());
    assert!(list(&record, "decisions").is_empty());
    assert!(
        list(&record, "ran")
            .iter()
            .all(|entry| text(entry, "reason") == REASON)
    );
    assert_eq!(
        field(&record, "window"),
        &value(r#"{"elapsed_seconds": 0.0, "started": 0, "unstarted": 5, "window_seconds": 0.0}"#)
    );
    let data = pyjson::parse(&brief_json(&record)).unwrap();
    assert_eq!(field(field(&data, "window"), "started"), &Value::uint(0));
    let md = brief_markdown(&record);
    assert!(md.contains(REASON));
    assert!(md.contains("## Window"));
}

#[test]
fn a_run_that_finishes_inside_the_window_records_no_window_closed_item() {
    let record = run_window(&backlog(), Some(WindowValue::from(100i64)));
    assert!(
        list(&record, "ran")
            .iter()
            .all(|entry| text(entry, "reason") != REASON)
    );
    assert_eq!(field(field(&record, "counts"), "finished"), &Value::uint(5));
    assert_eq!(
        field(&record, "window"),
        &value(
            r#"{"elapsed_seconds": 5.0, "started": 5, "unstarted": 0, "window_seconds": 100.0}"#
        )
    );
    assert_eq!(list(&record, "decisions").len(), 1);
}

fn markdown_window_block(md: &str) -> std::collections::BTreeMap<String, String> {
    let section = md.split("## Window").nth(1).unwrap();
    let mut values = std::collections::BTreeMap::new();
    for line in section.lines() {
        if let Some(rest) = line.strip_prefix("- ") {
            let (name, val) = rest.rsplit_once(' ').unwrap();
            values.insert(name.to_string(), val.to_string());
        }
    }
    values
}

#[test]
fn the_window_block_numbers_agree_in_both_brief_renderings() {
    let record = run_window(&backlog(), Some(WindowValue::from(2i64)));
    let data = pyjson::parse(&brief_json(&record)).unwrap();
    assert_eq!(
        field(&data, "window"),
        &value(r#"{"elapsed_seconds": 2.0, "started": 2, "unstarted": 3, "window_seconds": 2.0}"#)
    );
    let window = field(&data, "window").clone();
    let mut expected = std::collections::BTreeMap::new();
    expected.insert(
        "window seconds".to_string(),
        pyjson::py_str(field(&window, "window_seconds")),
    );
    expected.insert(
        "elapsed seconds".to_string(),
        pyjson::py_str(field(&window, "elapsed_seconds")),
    );
    expected.insert(
        "started".to_string(),
        pyjson::py_str(field(&window, "started")),
    );
    expected.insert(
        "unstarted".to_string(),
        pyjson::py_str(field(&window, "unstarted")),
    );
    assert_eq!(markdown_window_block(&brief_markdown(&record)), expected);
}

#[test]
fn the_window_is_measured_from_the_run_start_on_the_injected_clock() {
    let record = run_window_full(&backlog(), Some(WindowValue::from(2i64)), 1.0, 100.0);
    assert_eq!(
        field(&record, "window"),
        &value(r#"{"elapsed_seconds": 2.0, "started": 2, "unstarted": 3, "window_seconds": 2.0}"#)
    );
    assert_eq!(text(&by_id(&record)["task-1"], "status"), "finished");
}

#[test]
fn a_fractional_window_gates_on_the_fraction() {
    let record = run_window_full(&backlog(), Some(WindowValue::from(1.2f64)), 0.5, 0.0);
    let window = field(&record, "window");
    assert_eq!(field(window, "started"), &Value::uint(3));
    assert_eq!(field(window, "unstarted"), &Value::uint(2));
    assert_eq!(field(window, "elapsed_seconds"), &Value::Float(1.5));
}

#[test]
fn no_window_leaves_the_record_and_the_brief_without_a_window_block() {
    let record = run_window(&backlog(), None);
    assert!(record.get("window").is_none());
    let data = pyjson::parse(&brief_json(&record)).unwrap();
    assert!(data.get("window").is_none());
    assert!(!brief_markdown(&record).contains("## Window"));
}

#[test]
fn no_window_output_stays_byte_identical_to_the_committed_example() {
    let entries = load_backlog(&root().join("examples/backlog.json")).unwrap();
    let record = run_window(&entries, None);
    let expected_json = std::fs::read_to_string(root().join("examples/brief.json")).unwrap();
    let expected_md = std::fs::read_to_string(root().join("examples/brief.md")).unwrap();
    assert_eq!(brief_json(&record), expected_json);
    assert_eq!(brief_markdown(&record), expected_md);
}

#[test]
fn a_window_that_is_negative_or_not_a_number_is_refused_before_any_work() {
    let bad_windows: Vec<WindowValue> = vec![
        WindowValue::from(-1i64),
        WindowValue::from(-0.5f64),
        WindowValue::Text("2".to_string()),
        WindowValue::Text("soon".to_string()),
        WindowValue::Bool(true),
        WindowValue::from(f64::NAN),
        WindowValue::from(f64::INFINITY),
    ];
    for bad in bad_windows {
        let mut queue = WorkQueue::new(Rc::new(FakeClock::new(0.0)));
        let options = RunOptions {
            clock: Some(queue.clock.clone()),
            queue: Some(&mut queue),
            window: Some(bad),
            ..RunOptions::default()
        };
        let error = run_backlog(&backlog(), options).unwrap_err();
        assert!(matches!(error, RunError::Value(_)));
        assert!(queue.workers.is_empty());
        assert!(queue.claims.is_empty());
        assert!(queue.done.is_empty());
        assert_eq!(queue.stockouts, 0);
    }
}

#[test]
fn resolutions_apply_to_a_windowed_run_exactly_as_before() {
    let entries = vec![
        item("task-1", "alpha", "work", ""),
        item("task-2", "gate", "decision", "Ship the synthetic gate"),
        item("task-3", "omega", "work", ""),
    ];
    let record = run_window(&entries, Some(WindowValue::from(2i64)));
    let decision_ids: Vec<String> = list(&record, "decisions")
        .iter()
        .map(|decision| text(decision, "id"))
        .collect();
    assert_eq!(decision_ids, vec!["decision-task-2"]);
    assert_eq!(text(&by_id(&record)["task-3"], "reason"), REASON);
    let snapshot = record.clone();
    let resolution = value(
        r#"{"decision": "decision-task-2", "note": "ship it tonight", "verdict": "approve"}"#,
    );
    let out = apply_resolutions(&record, &[resolution]).unwrap();
    assert!(list(&out, "decisions").is_empty());
    let resolved = list(&out, "resolved");
    assert_eq!(text(&resolved[0], "id"), "decision-task-2");
    assert_eq!(text(&resolved[0], "verdict"), "approve");
    assert_eq!(text(&resolved[0], "note"), "ship it tonight");
    assert_eq!(
        field(&out, "decision_counts"),
        &value(r#"{"carried": 1, "queued": 0, "resolved": 1}"#)
    );
    assert_eq!(field(&out, "window"), field(&snapshot, "window"));
    for key in ["ran", "produced", "counts", "stock"] {
        assert_eq!(field(&out, key), field(&snapshot, key), "key {}", key);
    }
    assert_eq!(record, snapshot);
}

#[test]
fn a_decimal_or_fraction_window_is_accepted_as_the_number_it_names() {
    let record = run_window(&backlog(), Some(WindowValue::Decimal("2".to_string())));
    let window = field(&record, "window");
    assert_eq!(field(window, "window_seconds"), &Value::Float(2.0));
    assert_eq!(field(window, "started"), &Value::uint(2));
    let record = run_window(&backlog(), Some(WindowValue::Fraction(3, 2)));
    let window = field(&record, "window");
    assert_eq!(field(window, "window_seconds"), &Value::Float(1.5));
    assert_eq!(field(window, "started"), &Value::uint(2));
}

#[test]
fn a_window_too_large_to_convert_is_a_validation_error() {
    let huge = format!("1{}", "0".repeat(400));
    let mut queue = WorkQueue::new(Rc::new(FakeClock::new(0.0)));
    let options = RunOptions {
        clock: Some(queue.clock.clone()),
        queue: Some(&mut queue),
        window: Some(WindowValue::BigInt(huge)),
        ..RunOptions::default()
    };
    let error = run_backlog(&backlog(), options).unwrap_err();
    assert!(matches!(error, RunError::Value(_)));
}
