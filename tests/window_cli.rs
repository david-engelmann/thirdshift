//! The window command corpus, TS-32 and TS-33.

mod common;

use common::{field, list, root, run_cli, temp_dir, value, write_json};
use thirdshift::pyjson::{self, Value};

const REASON: &str = "the window closed before this item was started";

fn backlog() -> Value {
    value(
        r#"{"tasks": [
            {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
            {"id": "task-2", "title": "gate", "kind": "decision", "question": "Ship the synthetic gate"},
            {"id": "task-3", "title": "omega", "kind": "work", "question": ""}
        ]}"#,
    )
}

fn resolutions() -> Value {
    value(
        r#"{"resolutions": [
            {"decision": "decision-task-2", "verdict": "approve", "note": "ship it tonight"}
        ]}"#,
    )
}

fn text(entry: &Value, name: &str) -> String {
    entry
        .get(name)
        .and_then(|found| found.as_str())
        .unwrap_or("")
        .to_string()
}

#[test]
fn cli_run_with_a_window_records_the_window_block() {
    let dir = temp_dir("window-cli-run");
    let backlog_path = dir.join("backlog.json");
    write_json(&backlog_path, &backlog());
    let out = run_cli(
        &[
            "run",
            "--backlog",
            &backlog_path.display().to_string(),
            "--window-seconds",
            "2",
        ],
        &dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let record = pyjson::parse(&out.stdout).unwrap();
    assert_eq!(
        field(&record, "window"),
        &value(r#"{"elapsed_seconds": 2.0, "started": 2, "unstarted": 1, "window_seconds": 2.0}"#)
    );
    let tail: Vec<&Value> = list(&record, "ran")
        .iter()
        .filter(|entry| text(entry, "id") == "task-3")
        .collect();
    assert_eq!(text(tail[0], "status"), "unfinished");
    assert_eq!(text(tail[0], "reason"), REASON);
}

#[test]
fn cli_run_without_a_window_has_no_window_block() {
    let root = root();
    let out = run_cli(&["run", "--backlog", "examples/backlog.json"], &root);
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let record = pyjson::parse(&out.stdout).unwrap();
    assert!(record.get("window").is_none());
}

#[test]
fn cli_brief_with_a_window_matches_the_committed_windowed_example() {
    let root = root();
    let dir = temp_dir("window-cli-brief");
    let md_path = dir.join("windowed-brief.md");
    let json_path = dir.join("windowed-brief.json");
    let out = run_cli(
        &[
            "brief",
            "--backlog",
            "examples/backlog.json",
            "--window-seconds",
            "4",
            "--md",
            &md_path.display().to_string(),
            "--json",
            &json_path.display().to_string(),
        ],
        &root,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    assert_eq!(
        std::fs::read_to_string(&md_path).unwrap(),
        std::fs::read_to_string(root.join("examples/windowed-brief.md")).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(&json_path).unwrap(),
        std::fs::read_to_string(root.join("examples/windowed-brief.json")).unwrap()
    );
}

fn refused_window(step: &str, bad: &str, message: &str) {
    let dir = temp_dir("window-cli-refused");
    let backlog_path = dir.join("backlog.json");
    write_json(&backlog_path, &backlog());
    let out = run_cli(
        &[
            step,
            "--backlog",
            &backlog_path.display().to_string(),
            "--window-seconds",
            bad,
        ],
        &dir,
    );
    assert_eq!(out.code, 2, "step {} bad {}", step, bad);
    assert_eq!(out.stdout, "");
    assert_eq!(out.stderr, format!("error - {}\n", message));
}

#[test]
fn cli_run_refuses_a_negative_window() {
    refused_window("run", "-1", "window seconds must be a non-negative number, -1");
}

#[test]
fn cli_run_refuses_a_non_numeric_window() {
    refused_window("run", "soon", "window seconds is not a number, soon");
}

#[test]
fn cli_brief_refuses_a_negative_window() {
    refused_window("brief", "-1", "window seconds must be a non-negative number, -1");
}

#[test]
fn cli_brief_refuses_a_non_numeric_window() {
    refused_window("brief", "soon", "window seconds is not a number, soon");
}

#[test]
fn cli_run_refuses_a_dash_inf_window() {
    refused_window("run", "-inf", "window seconds must be a non-negative number, -inf");
}

#[test]
fn cli_run_refuses_a_dash_nan_window() {
    refused_window("run", "-nan", "window seconds must be a non-negative number, -nan");
}

#[test]
fn cli_run_refuses_a_dash_negative_window() {
    refused_window("run", "-2.5", "window seconds must be a non-negative number, -2.5");
}

#[test]
fn cli_brief_refuses_a_dash_inf_window() {
    refused_window("brief", "-inf", "window seconds must be a non-negative number, -inf");
}

#[test]
fn cli_brief_refuses_a_dash_nan_window() {
    refused_window("brief", "-nan", "window seconds must be a non-negative number, -nan");
}

#[test]
fn cli_brief_refuses_a_dash_negative_window() {
    refused_window("brief", "-2.5", "window seconds must be a non-negative number, -2.5");
}

#[test]
fn cli_resolve_over_the_committed_windowed_examples_matches_the_committed_record() {
    let root = root();
    let out = run_cli(
        &[
            "resolve",
            "--brief",
            "examples/windowed-brief.json",
            "--resolutions",
            "examples/resolutions.json",
        ],
        &root,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let expected = std::fs::read_to_string(root.join("examples/windowed-resolved.json")).unwrap();
    assert_eq!(out.stdout, expected);
    let record = pyjson::parse(&out.stdout).unwrap();
    assert_eq!(
        field(&record, "window"),
        &value(r#"{"elapsed_seconds": 4.0, "started": 4, "unstarted": 1, "window_seconds": 4.0}"#)
    );
    assert_eq!(
        field(&record, "decision_counts"),
        &value(r#"{"carried": 1, "queued": 0, "resolved": 1}"#)
    );
}

#[test]
fn cli_resolve_applies_to_a_windowed_brief_built_by_the_cli() {
    let dir = temp_dir("window-cli-resolve");
    let backlog_path = dir.join("backlog.json");
    write_json(&backlog_path, &backlog());
    let brief_path = dir.join("brief.json");
    let brief = run_cli(
        &[
            "brief",
            "--backlog",
            &backlog_path.display().to_string(),
            "--window-seconds",
            "2",
            "--json",
            &brief_path.display().to_string(),
        ],
        &dir,
    );
    assert_eq!(brief.code, 0, "stderr {}", brief.stderr);
    let resolutions_path = dir.join("resolutions.json");
    write_json(&resolutions_path, &resolutions());
    let out = run_cli(
        &[
            "resolve",
            "--brief",
            &brief_path.display().to_string(),
            "--resolutions",
            &resolutions_path.display().to_string(),
        ],
        &dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let record = pyjson::parse(&out.stdout).unwrap();
    let resolved = list(&record, "resolved");
    assert_eq!(text(&resolved[0], "verdict"), "approve");
    let window = field(&record, "window");
    assert_eq!(field(window, "started"), &Value::uint(2));
    assert_eq!(field(window, "unstarted"), &Value::uint(1));
}
