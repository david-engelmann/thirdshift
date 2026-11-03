//! The brief corpus, TS-8 and TS-9, one brief in two forms.

mod common;

use std::collections::BTreeMap;
use std::rc::Rc;

use common::{field, list, value};
use thirdshift::pyjson::{self, Value};
use thirdshift::{brief_json, brief_markdown, run_backlog, FakeClock, Item, RunOptions};

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
    ]
}

fn run(entries: &[Value]) -> Value {
    let options = RunOptions {
        clock: Some(Rc::new(FakeClock::new(0.0))),
        ..RunOptions::default()
    };
    run_backlog(entries, options).unwrap()
}

fn make_brief() -> Value {
    run(&backlog())
}

fn text(entry: &Value, name: &str) -> String {
    entry
        .get(name)
        .and_then(|found| found.as_str())
        .unwrap_or("")
        .to_string()
}

#[test]
fn json_brief_lists_ran_produced_and_decisions() {
    let data = pyjson::parse(&brief_json(&make_brief())).unwrap();
    let ids: Vec<String> = list(&data, "ran")
        .iter()
        .map(|entry| text(entry, "id"))
        .collect();
    assert_eq!(ids, vec!["task-1", "task-2", "task-3"]);
    assert_eq!(field(&data, "produced"), &value(r#"["completed task-1"]"#));
    let decisions = list(&data, "decisions");
    assert_eq!(text(&decisions[0], "question"), "Ship the synthetic gate");
    assert_eq!(
        field(&decisions[0], "options"),
        &value(r#"["approve", "reject"]"#)
    );
}

#[test]
fn markdown_brief_carries_the_same_content_as_the_json() {
    let brief = make_brief();
    let md = brief_markdown(&brief);
    let data = pyjson::parse(&brief_json(&brief)).unwrap();
    for entry in list(&data, "ran") {
        assert!(md.contains(&text(entry, "id")));
        assert!(md.contains(&text(entry, "worker")));
        assert!(md.contains(&text(entry, "status")));
    }
    for output in list(&data, "produced") {
        assert!(md.contains(output.as_str().unwrap()));
    }
    for decision in list(&data, "decisions") {
        assert!(md.contains(&text(decision, "id")));
        assert!(md.contains(&text(decision, "question")));
    }
    assert!(md.contains("approve"));
    assert!(md.contains("reject"));
}

#[test]
fn markdown_brief_shows_the_reason_for_unfinished_work() {
    let md = brief_markdown(&make_brief());
    assert!(md.contains("stub worker could not complete this task"));
}

#[test]
fn two_runs_over_the_same_backlog_give_byte_identical_briefs() {
    let first = make_brief();
    let second = make_brief();
    assert_eq!(brief_json(&first), brief_json(&second));
    assert_eq!(brief_markdown(&first), brief_markdown(&second));
}

#[test]
fn brief_counts_match_the_record() {
    let data = pyjson::parse(&brief_json(&make_brief())).unwrap();
    assert_eq!(
        field(&data, "counts"),
        &value(r#"{"backlog": 3, "finished": 2, "recorded": 3, "unfinished": 1}"#)
    );
}

fn markdown_stock_block(md: &str) -> BTreeMap<String, String> {
    let section = md.split("## Stock").nth(1).unwrap();
    let mut values = BTreeMap::new();
    for line in section.lines() {
        if let Some(rest) = line.strip_prefix("- ") {
            let (name, val) = rest.rsplit_once(' ').unwrap();
            values.insert(name.to_string(), val.to_string());
        }
    }
    values
}

#[test]
fn the_two_brief_forms_carry_the_same_content() {
    let brief = make_brief();
    let data = pyjson::parse(&brief_json(&brief)).unwrap();
    let md = brief_markdown(&brief);
    for entry in list(&data, "ran") {
        for key in ["id", "title", "worker", "status"] {
            assert!(md.contains(&text(entry, key)), "missing {} in md", key);
        }
        let output = text(entry, "output");
        if !output.is_empty() {
            assert!(md.contains(&output));
        }
        let reason = text(entry, "reason");
        if !reason.is_empty() {
            assert!(md.contains(&reason));
        }
    }
    for output in list(&data, "produced") {
        assert!(md.contains(output.as_str().unwrap()));
    }
    for decision in list(&data, "decisions") {
        assert!(md.contains(&text(decision, "id")));
        assert!(md.contains(&text(decision, "item")));
        assert!(md.contains(&text(decision, "question")));
    }
    let labels = [
        ("workers", "workers"),
        ("stocked_at_start", "stocked at start"),
        ("stockouts_recorded", "stockouts recorded"),
    ];
    let stock = field(&data, "stock");
    let expected: BTreeMap<String, String> = labels
        .iter()
        .map(|(key, label)| {
            (
                label.to_string(),
                pyjson::py_str(field(stock, key)),
            )
        })
        .collect();
    assert_eq!(markdown_stock_block(&md), expected);
}

#[test]
fn brief_stock_block_reports_the_run_not_a_standing_stockout() {
    let data = pyjson::parse(&brief_json(&make_brief())).unwrap();
    let stock = field(&data, "stock");
    assert_eq!(
        stock,
        &value(r#"{"stockouts_recorded": 0, "stocked_at_start": 3, "workers": 2}"#)
    );
    assert!(stock.get("stockout").is_none());
}

#[test]
fn briefs_do_not_depend_on_backlog_entry_order() {
    let four = vec![
        item("task-1", "alpha", "work", ""),
        item("task-2", "beta", "work", ""),
        item("task-3", "gate", "decision", "Ship the synthetic gate"),
        item("task-4", "omega", "work", ""),
    ];
    let forward = run(&four);
    let backward = run(&four.iter().rev().cloned().collect::<Vec<_>>());
    let shuffled = run(&[four[2].clone(), four[0].clone(), four[3].clone(), four[1].clone()]);
    assert_eq!(backward, forward);
    assert_eq!(shuffled, forward);
    assert_eq!(brief_json(&backward), brief_json(&forward));
    assert_eq!(brief_json(&shuffled), brief_json(&forward));
    assert_eq!(brief_markdown(&backward), brief_markdown(&forward));
    assert_eq!(brief_markdown(&shuffled), brief_markdown(&forward));
}
