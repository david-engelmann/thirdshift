//! The resolution corpus, TS-23 to TS-26 and TS-28.

mod common;

use std::rc::Rc;

use common::{field, list, temp_dir, value};
use thirdshift::pyjson::{self, Value};
use thirdshift::{FakeClock, Item, RunOptions, apply_resolutions, load_resolutions, run_backlog};

fn item(item_id: &str, title: &str, kind: &str, question: &str) -> Value {
    let mut entry = Item::new(item_id, title);
    entry.kind = kind.to_string();
    entry.question = question.to_string();
    entry.to_value()
}

fn make_record() -> Value {
    let entries = vec![
        item("task-1", "alpha", "work", ""),
        item(
            "task-2",
            "first gate",
            "decision",
            "Ship the synthetic gate",
        ),
        item(
            "task-3",
            "second gate",
            "decision",
            "Ship the second synthetic gate",
        ),
    ];
    let options = RunOptions {
        clock: Some(Rc::new(FakeClock::new(0.0))),
        ..RunOptions::default()
    };
    run_backlog(&entries, options).unwrap()
}

fn approve_first() -> Value {
    value(r#"{"decision": "decision-task-2", "note": "ship it tonight", "verdict": "approve"}"#)
}

fn reject_second() -> Value {
    value(
        r#"{"decision": "decision-task-3", "note": "hold for the next window", "verdict": "reject"}"#,
    )
}

fn text(entry: &Value, name: &str) -> String {
    entry
        .get(name)
        .and_then(|found| found.as_str())
        .unwrap_or("")
        .to_string()
}

fn decision_ids(entries: &[Value]) -> Vec<String> {
    entries.iter().map(|entry| text(entry, "id")).collect()
}

#[test]
fn a_resolution_is_applied_and_visible() {
    let out = apply_resolutions(&make_record(), &[approve_first()]).unwrap();
    let resolved = list(&out, "resolved");
    assert_eq!(resolved.len(), 1);
    let entry = &resolved[0];
    assert_eq!(text(entry, "id"), "decision-task-2");
    assert_eq!(text(entry, "item"), "task-2");
    assert_eq!(text(entry, "question"), "Ship the synthetic gate");
    assert_eq!(text(entry, "verdict"), "approve");
    assert_eq!(text(entry, "note"), "ship it tonight");
    assert_eq!(
        decision_ids(list(&out, "decisions")),
        vec!["decision-task-3"]
    );
}

#[test]
fn an_unnamed_decision_stays_queued_exactly_as_the_brief_carried_it() {
    let record = make_record();
    let out = apply_resolutions(&record, &[approve_first()]).unwrap();
    let carried: Vec<Value> = list(&record, "decisions")
        .iter()
        .filter(|decision| text(decision, "id") == "decision-task-3")
        .cloned()
        .collect();
    assert_eq!(list(&out, "decisions"), &carried);
}

#[test]
fn decision_counts_add_up_for_partial_full_and_empty_resolution() {
    let record = make_record();
    let partial = apply_resolutions(&record, &[approve_first()]).unwrap();
    assert_eq!(
        field(&partial, "decision_counts"),
        &value(r#"{"carried": 2, "queued": 1, "resolved": 1}"#)
    );
    let full = apply_resolutions(&record, &[approve_first(), reject_second()]).unwrap();
    assert_eq!(
        field(&full, "decision_counts"),
        &value(r#"{"carried": 2, "queued": 0, "resolved": 2}"#)
    );
    assert!(list(&full, "decisions").is_empty());
    let empty = apply_resolutions(&record, &[]).unwrap();
    assert_eq!(
        field(&empty, "decision_counts"),
        &value(r#"{"carried": 2, "queued": 2, "resolved": 0}"#)
    );
    assert!(list(&empty, "resolved").is_empty());
    for out in [&partial, &full, &empty] {
        let counts = field(out, "decision_counts");
        let queued = field(counts, "queued").clone();
        let resolved = field(counts, "resolved").clone();
        let carried = field(counts, "carried").clone();
        if let (Value::Int(q), Value::Int(r), Value::Int(c)) = (&queued, &resolved, &carried) {
            assert_eq!(
                q.parse::<i64>().unwrap() + r.parse::<i64>().unwrap(),
                c.parse::<i64>().unwrap()
            );
        } else {
            panic!("decision counts are not integers");
        }
    }
}

#[test]
fn the_resolved_list_follows_the_brief_order_not_the_file_order() {
    let out = apply_resolutions(&make_record(), &[reject_second(), approve_first()]).unwrap();
    assert_eq!(
        decision_ids(list(&out, "resolved")),
        vec!["decision-task-2", "decision-task-3"]
    );
}

#[test]
fn a_resolution_naming_a_decision_the_brief_does_not_carry_is_refused() {
    let record = make_record();
    let snapshot = record.clone();
    let stranger =
        value(r#"{"decision": "decision-task-9", "note": "who is this", "verdict": "approve"}"#);
    let error = apply_resolutions(&record, &[stranger]).unwrap_err();
    assert!(error.0.contains("decision-task-9"), "message {}", error.0);
    assert_eq!(record, snapshot);
}

#[test]
fn a_decision_named_twice_in_one_resolutions_list_is_refused() {
    let record = make_record();
    let snapshot = record.clone();
    let error = apply_resolutions(&record, &[approve_first(), approve_first()]).unwrap_err();
    assert!(error.0.contains("decision-task-2"), "message {}", error.0);
    assert_eq!(record, snapshot);
}

#[test]
fn a_verdict_that_is_neither_approve_nor_reject_is_refused() {
    let record = make_record();
    let snapshot = record.clone();
    let shrug =
        value(r#"{"decision": "decision-task-2", "note": "undecided", "verdict": "maybe"}"#);
    let error = apply_resolutions(&record, &[shrug]).unwrap_err();
    assert!(error.0.contains("decision-task-2"), "message {}", error.0);
    assert_eq!(record, snapshot);
}

#[test]
fn a_refusal_after_a_valid_resolution_still_applies_nothing() {
    let record = make_record();
    let snapshot = record.clone();
    let stranger =
        value(r#"{"decision": "decision-task-9", "note": "who is this", "verdict": "approve"}"#);
    assert!(apply_resolutions(&record, &[approve_first(), stranger]).is_err());
    assert_eq!(record, snapshot);
}

#[test]
fn a_resolution_that_is_not_an_object_is_refused() {
    assert!(apply_resolutions(&make_record(), &[Value::Str("approve".to_string())]).is_err());
}

#[test]
fn a_resolution_missing_its_note_is_refused() {
    let bare = value(r#"{"decision": "decision-task-2", "verdict": "approve"}"#);
    assert!(apply_resolutions(&make_record(), &[bare]).is_err());
}

#[test]
fn a_resolution_with_a_wrongly_typed_verdict_is_refused() {
    let bad = value(r#"{"decision": "decision-task-2", "note": "typed wrong", "verdict": 1}"#);
    assert!(apply_resolutions(&make_record(), &[bad]).is_err());
}

#[test]
fn a_record_with_no_decisions_list_is_refused() {
    let record = value(r#"{"ran": []}"#);
    assert!(apply_resolutions(&record, &[approve_first()]).is_err());
}

#[test]
fn the_run_record_is_byte_identical_after_resolution() {
    let record = make_record();
    let snapshot = record.clone();
    let out = apply_resolutions(&record, &[approve_first(), reject_second()]).unwrap();
    let run_part = |source: &Value| {
        let part = Value::object(vec![
            ("ran".to_string(), field(source, "ran").clone()),
            ("produced".to_string(), field(source, "produced").clone()),
            ("counts".to_string(), field(source, "counts").clone()),
            ("stock".to_string(), field(source, "stock").clone()),
        ]);
        pyjson::dumps_compact(&part)
    };
    assert_eq!(run_part(&snapshot), run_part(&out));
    assert_eq!(record, snapshot);
}

fn write_resolutions(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
    let path = dir.join("resolutions.json");
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_valid_resolutions_file_loads_its_entries() {
    let dir = temp_dir("resolutions-valid");
    let envelope = Value::object(vec![(
        "resolutions".to_string(),
        Value::List(vec![approve_first(), reject_second()]),
    )]);
    let path = write_resolutions(&dir, &pyjson::dumps_compact(&envelope));
    assert_eq!(
        load_resolutions(&path).unwrap(),
        vec![approve_first(), reject_second()]
    );
}

#[test]
fn a_resolutions_file_with_invalid_json_fails_at_load() {
    let dir = temp_dir("resolutions-json");
    let path = write_resolutions(&dir, "{not json");
    assert!(load_resolutions(&path).is_err());
}

#[test]
fn a_resolutions_object_with_no_resolutions_list_fails_at_load() {
    let dir = temp_dir("resolutions-shape");
    let path = write_resolutions(&dir, r#"{"verdicts": []}"#);
    assert!(load_resolutions(&path).is_err());
}

#[test]
fn a_resolutions_file_that_is_a_bare_list_fails_at_load() {
    let dir = temp_dir("resolutions-bare");
    let text = pyjson::dumps_compact(&Value::List(vec![approve_first()]));
    let path = write_resolutions(&dir, &text);
    assert!(load_resolutions(&path).is_err());
}

#[test]
fn a_missing_resolutions_file_fails_at_load() {
    let dir = temp_dir("resolutions-missing");
    assert!(load_resolutions(&dir.join("nope.json")).is_err());
}

fn decision(decision_id: &str) -> Value {
    Value::object(vec![
        ("id".to_string(), Value::Str(decision_id.to_string())),
        ("item".to_string(), Value::Str("task-x".to_string())),
        (
            "question".to_string(),
            Value::Str("Ship the synthetic gate".to_string()),
        ),
        (
            "options".to_string(),
            Value::List(vec![
                Value::Str("approve".to_string()),
                Value::Str("reject".to_string()),
            ]),
        ),
    ])
}

fn record_with(decisions: Vec<Value>) -> Value {
    let mut record = make_record();
    record.set("decisions", Value::List(decisions));
    record
}

fn one_resolution() -> Value {
    value(r#"{"decision": "d1", "note": "n", "verdict": "approve"}"#)
}

#[test]
fn a_brief_with_a_non_string_decision_id_is_refused() {
    let mut broken = decision("d2");
    broken.set("id", value(r#"["x"]"#));
    let record = record_with(vec![decision("d1"), broken]);
    assert!(apply_resolutions(&record, &[one_resolution()]).is_err());
}

#[test]
fn a_brief_carrying_one_decision_twice_is_refused() {
    let record = record_with(vec![decision("d1"), decision("d1")]);
    assert!(apply_resolutions(&record, &[one_resolution()]).is_err());
}

#[test]
fn a_brief_with_a_non_object_decision_is_refused() {
    let record = record_with(vec![Value::Str("junk".to_string()), decision("d1")]);
    assert!(apply_resolutions(&record, &[one_resolution()]).is_err());
}

#[test]
fn a_brief_with_an_id_less_decision_is_refused() {
    let mut entry = decision("d1");
    // Rebuild the decision without its id field.
    let mut rebuilt = Vec::new();
    if let Value::Object(pairs) = &entry {
        for (key, val) in pairs {
            if key != "id" {
                rebuilt.push((key.clone(), val.clone()));
            }
        }
    }
    entry = Value::Object(rebuilt);
    let record = record_with(vec![entry]);
    assert!(apply_resolutions(&record, &[]).is_err());
}

#[test]
fn the_resolved_record_shares_no_mutable_state_with_its_input() {
    let record = make_record();
    let mut out = apply_resolutions(&record, &[approve_first()]).unwrap();
    // The mutations are the point of the test, so a record that
    // does not carry the lists fails here instead of passing
    // through a discarded Option.
    let decisions = out
        .get_mut("decisions")
        .and_then(|decisions| decisions.as_list_mut())
        .expect("the resolved record carries its decisions list");
    decisions[0].set("question", Value::Str("mutated".to_string()));
    let ran = out
        .get_mut("ran")
        .and_then(|ran| ran.as_list_mut())
        .expect("the resolved record carries its ran list");
    ran.push(value(r#"{"item": "mutant"}"#));
    let fresh = make_record();
    assert_eq!(record, fresh);
}
