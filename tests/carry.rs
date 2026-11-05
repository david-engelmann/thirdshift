//! The carry, TS-42 to TS-47, at the library surface.

mod common;

use common::value;
use thirdshift::pyjson::Value;
use thirdshift::runner::{RunOptions, WindowValue, run_backlog};
use thirdshift::{apply_resolutions, carry_backlog};

fn entries() -> Vec<Value> {
    value(
        r#"[
            {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
            {"id": "task-2", "title": "beta", "kind": "fail", "question": ""},
            {"id": "task-3", "title": "gate", "kind": "decision", "question": "Ship the synthetic gate"},
            {"id": "task-4", "title": "gamma", "kind": "work", "question": ""}
        ]"#,
    )
    .as_list()
    .expect("entries list")
    .clone()
}

fn record_for(entries: &[Value]) -> Value {
    run_backlog(entries, RunOptions::default()).expect("run records")
}

fn carried_ids(carried: &[Value]) -> Vec<&str> {
    carried
        .iter()
        .map(|entry| entry.get("id").and_then(Value::as_str).unwrap_or(""))
        .collect()
}

#[test]
fn carry_holds_exactly_the_unfinished_items() {
    let entries = entries();
    let record = record_for(&entries);
    let carried = carry_backlog(&entries, &record).expect("carry joins");
    assert_eq!(carried_ids(&carried), vec!["task-2"]);
    assert_eq!(
        carried[0], entries[1],
        "the carried entry is the backlog entry"
    );
}

#[test]
fn carry_copies_every_field_verbatim() {
    let rich = value(
        r#"[
            {"id": "task-1", "title": "alpha", "kind": "fail", "question": "",
             "priority": 3, "tags": ["night", "cold"], "note": {"pages": 2, "signed": true}},
            {"id": "task-2", "title": "beta", "kind": "work", "question": "", "estimate": 1.5}
        ]"#,
    )
    .as_list()
    .expect("entries list")
    .clone();
    let record = record_for(&rich);
    let carried = carry_backlog(&rich, &record).expect("carry joins");
    assert_eq!(carried.len(), 1);
    assert_eq!(
        carried[0], rich[0],
        "every field the entry carried survives, in value and in shape"
    );
}

#[test]
fn carry_stands_in_backlog_order_over_a_shuffled_record() {
    let entries = entries();
    let record = value(
        r#"{"ran": [
            {"id": "task-4", "title": "gamma", "worker": "", "status": "unfinished", "output": "", "reason": "r"},
            {"id": "task-2", "title": "beta", "worker": "worker-1", "status": "unfinished", "output": "", "reason": "r"},
            {"id": "task-3", "title": "gate", "worker": "worker-2", "status": "finished", "output": "", "reason": ""},
            {"id": "task-1", "title": "alpha", "worker": "worker-1", "status": "finished", "output": "completed task-1", "reason": ""}
        ]}"#,
    );
    let carried = carry_backlog(&entries, &record).expect("carry joins");
    assert_eq!(carried_ids(&carried), vec!["task-2", "task-4"]);
}

#[test]
fn carry_from_a_record_that_finished_everything_is_an_empty_handoff() {
    let entries = value(
        r#"[
            {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
            {"id": "task-2", "title": "beta", "kind": "work", "question": ""}
        ]"#,
    )
    .as_list()
    .expect("entries list")
    .clone();
    let record = record_for(&entries);
    let carried = carry_backlog(&entries, &record).expect("carry joins");
    assert!(carried.is_empty(), "nothing unfinished, nothing carried");
    let next = run_backlog(&carried, RunOptions::default()).expect("empty backlog runs");
    assert_eq!(
        next.get("counts"),
        Some(&value(
            r#"{"backlog": 0, "finished": 0, "recorded": 0, "unfinished": 0}"#
        )),
        "the empty handoff runs as a backlog in its own right"
    );
}

#[test]
fn carry_queues_a_carried_decision_again_with_its_question() {
    let entries = value(
        r#"[
            {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
            {"id": "task-2", "title": "gate", "kind": "decision", "question": "Ship the synthetic gate"}
        ]"#,
    )
    .as_list()
    .expect("entries list")
    .clone();
    let options = RunOptions {
        window: Some(WindowValue::Float(0.0)),
        ..RunOptions::default()
    };
    let record = run_backlog(&entries, options).expect("windowed run records");
    let carried = carry_backlog(&entries, &record).expect("carry joins");
    assert_eq!(carried_ids(&carried), vec!["task-1", "task-2"]);
    let next = run_backlog(&carried, RunOptions::default()).expect("carried backlog runs");
    let decisions = next.get("decisions").expect("decisions block").clone();
    let decisions = decisions.as_list().expect("decisions list");
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].get("question"),
        Some(&Value::Str("Ship the synthetic gate".to_string())),
        "the carried decision queues again with its question intact"
    );
    let brief = thirdshift::brief_markdown(&next);
    assert!(
        brief.contains("Ship the synthetic gate"),
        "the next brief shows the question"
    );
}

#[test]
fn carry_takes_the_items_the_window_left_unstarted() {
    let entries = value(
        r#"[
            {"id": "task-1", "title": "alpha", "kind": "fail", "question": ""},
            {"id": "task-2", "title": "beta", "kind": "work", "question": ""},
            {"id": "task-3", "title": "gamma", "kind": "work", "question": ""}
        ]"#,
    )
    .as_list()
    .expect("entries list")
    .clone();
    let options = RunOptions {
        window: Some(WindowValue::Float(1.0)),
        ..RunOptions::default()
    };
    let record = run_backlog(&entries, options).expect("windowed run records");
    let carried = carry_backlog(&entries, &record).expect("carry joins");
    assert_eq!(
        carried_ids(&carried),
        vec!["task-1", "task-2", "task-3"],
        "an item the run started and left unfinished and the items the window left unstarted all carry"
    );
}

#[test]
fn carry_from_a_resolved_record_matches_the_record_it_came_from() {
    let entries = entries();
    let record = record_for(&entries);
    let resolutions = value(
        r#"[
            {"decision": "decision-task-3", "verdict": "approve", "note": "ship it tonight"}
        ]"#,
    )
    .as_list()
    .expect("resolutions list")
    .clone();
    let resolved = apply_resolutions(&record, &resolutions).expect("record resolves");
    let from_plain = carry_backlog(&entries, &record).expect("carry joins");
    let from_resolved = carry_backlog(&entries, &resolved).expect("carry joins");
    assert_eq!(from_plain, from_resolved);
}

fn refusal(entries: &[Value], record: &Value) -> String {
    carry_backlog(entries, record)
        .expect_err("carry refuses")
        .to_string()
}

#[test]
fn carry_refuses_an_item_the_record_names_and_the_backlog_does_not_carry() {
    let entries = entries();
    let record = value(
        r#"{"ran": [
            {"id": "task-1", "status": "finished"},
            {"id": "task-2", "status": "unfinished"},
            {"id": "task-3", "status": "finished"},
            {"id": "task-4", "status": "finished"},
            {"id": "task-9", "status": "unfinished"}
        ]}"#,
    );
    assert_eq!(
        refusal(&entries, &record),
        "record names item task-9, which the backlog does not carry"
    );
}

#[test]
fn carry_refusal_names_the_first_unknown_item_in_record_order() {
    let entries = entries();
    let record = value(
        r#"{"ran": [
            {"id": "task-8", "status": "unfinished"},
            {"id": "task-1", "status": "finished"},
            {"id": "task-2", "status": "unfinished"},
            {"id": "task-3", "status": "finished"},
            {"id": "task-4", "status": "finished"},
            {"id": "task-9", "status": "unfinished"}
        ]}"#,
    );
    assert_eq!(
        refusal(&entries, &record),
        "record names item task-8, which the backlog does not carry"
    );
}

#[test]
fn carry_refuses_an_item_the_backlog_carries_and_the_record_does_not_mention() {
    let entries = entries();
    let record = value(
        r#"{"ran": [
            {"id": "task-1", "status": "finished"},
            {"id": "task-2", "status": "unfinished"},
            {"id": "task-3", "status": "finished"}
        ]}"#,
    );
    assert_eq!(
        refusal(&entries, &record),
        "backlog carries item task-4, which the record does not mention"
    );
}

#[test]
fn carry_refusal_names_the_first_unmentioned_item_in_backlog_order() {
    let entries = entries();
    let record = value(
        r#"{"ran": [
            {"id": "task-2", "status": "unfinished"},
            {"id": "task-4", "status": "finished"}
        ]}"#,
    );
    assert_eq!(
        refusal(&entries, &record),
        "backlog carries item task-1, which the record does not mention"
    );
}

#[test]
fn carry_record_refusal_fires_ahead_of_the_backlog_refusal() {
    let entries = entries();
    let record = value(
        r#"{"ran": [
            {"id": "task-1", "status": "finished"},
            {"id": "task-9", "status": "unfinished"}
        ]}"#,
    );
    assert_eq!(
        refusal(&entries, &record),
        "record names item task-9, which the backlog does not carry",
        "the record side is checked before the backlog side is missed"
    );
}

#[test]
fn carry_refuses_a_ran_entry_that_is_not_an_object() {
    let entries = entries();
    let record = value(r#"{"ran": ["task-1"]}"#);
    assert_eq!(
        refusal(&entries, &record),
        "brief record carries a ran entry that is not an object"
    );
}

#[test]
fn carry_refuses_a_ran_entry_whose_id_is_not_a_non_empty_string() {
    let entries = entries();
    for record in [
        value(r#"{"ran": [{"id": 7, "status": "finished"}]}"#),
        value(r#"{"ran": [{"id": "", "status": "finished"}]}"#),
        value(r#"{"ran": [{"status": "finished"}]}"#),
    ] {
        assert_eq!(
            refusal(&entries, &record),
            "brief record carries a ran entry whose id is not a non-empty string"
        );
    }
}

#[test]
fn carry_refuses_a_ran_entry_whose_status_is_outside_the_vocabulary() {
    let entries = entries();
    for record in [
        value(r#"{"ran": [{"id": "task-1", "status": "stalled"}]}"#),
        value(r#"{"ran": [{"id": "task-1", "status": 1}]}"#),
        value(r#"{"ran": [{"id": "task-1"}]}"#),
    ] {
        assert_eq!(
            refusal(&entries, &record),
            "brief record carries item task-1, whose status is neither finished nor unfinished"
        );
    }
}

#[test]
fn carry_refuses_an_item_named_twice_in_the_record() {
    let entries = entries();
    let record = value(
        r#"{"ran": [
            {"id": "task-1", "status": "finished"},
            {"id": "task-1", "status": "unfinished"}
        ]}"#,
    );
    assert_eq!(
        refusal(&entries, &record),
        "brief record carries item task-1 twice"
    );
}

#[test]
fn record_shape_refusals_fire_ahead_of_record_duplicates() {
    let entries = entries();
    let record = value(
        r#"{"ran": [
            {"id": "task-1", "status": "finished"},
            {"id": "task-1", "status": "finished"},
            {"id": "task-2", "status": "stalled"}
        ]}"#,
    );
    assert_eq!(
        refusal(&entries, &record),
        "brief record carries item task-2, whose status is neither finished nor unfinished"
    );
}

#[test]
fn backlog_naming_refusals_fire_ahead_of_backlog_duplicates() {
    let entries = value(
        r#"[
            {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
            {"id": "task-1", "title": "alpha again", "kind": "work", "question": ""},
            {"title": "nameless", "kind": "work", "question": ""}
        ]"#,
    )
    .as_list()
    .expect("entries list")
    .clone();
    let record = value(r#"{"ran": [{"id": "task-1", "status": "finished"}]}"#);
    assert_eq!(
        refusal(&entries, &record),
        "backlog entry carries no item id"
    );
}

#[test]
fn carry_refuses_a_backlog_entry_with_no_item_id() {
    let record = value(r#"{"ran": []}"#);
    for entries in [
        value(r#"[{"title": "alpha", "kind": "work", "question": ""}]"#),
        value(r#"[{"id": "", "title": "alpha"}]"#),
        value(r#"[{"id": 7, "title": "alpha"}]"#),
        value(r#"["just text"]"#),
    ] {
        let entries = entries.as_list().expect("entries list").clone();
        assert_eq!(
            refusal(&entries, &record),
            "backlog entry carries no item id"
        );
    }
}

#[test]
fn carry_refuses_an_item_named_twice_in_the_backlog() {
    let entries = value(
        r#"[
            {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
            {"id": "task-1", "title": "alpha again", "kind": "work", "question": ""}
        ]"#,
    )
    .as_list()
    .expect("entries list")
    .clone();
    let record = value(r#"{"ran": [{"id": "task-1", "status": "finished"}]}"#);
    assert_eq!(
        refusal(&entries, &record),
        "backlog carries item task-1 twice"
    );
}

#[test]
fn carry_shape_refusals_fire_ahead_of_backlog_naming_refusals() {
    let entries = value(r#"[{"title": "alpha"}]"#)
        .as_list()
        .expect("entries list")
        .clone();
    let record = value(r#"{"ran": [{"id": "task-1", "status": "stalled"}]}"#);
    assert_eq!(
        refusal(&entries, &record),
        "brief record carries item task-1, whose status is neither finished nor unfinished",
        "the record shape is validated before the backlog is named"
    );
}

#[test]
fn carry_refuses_a_record_with_no_ran_list() {
    let entries = entries();
    for record in [value(r#"{"decisions": []}"#), value(r#"[1, 2]"#)] {
        assert_eq!(
            refusal(&entries, &record),
            "brief record carries no ran list"
        );
    }
}
