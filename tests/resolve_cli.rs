//! The resolve command corpus, TS-27.

mod common;

use std::path::PathBuf;

use common::{field, list, root, run_cli, temp_dir, value, write_json};
use thirdshift::pyjson::{self, Value};

fn backlog() -> Value {
    value(
        r#"{"tasks": [
            {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
            {"id": "task-2", "title": "gate", "kind": "decision", "question": "Ship the synthetic gate"}
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

fn make_brief_json(dir: &std::path::Path) -> PathBuf {
    let backlog_path = dir.join("backlog.json");
    write_json(&backlog_path, &backlog());
    let brief_path = dir.join("brief.json");
    let out = run_cli(
        &[
            "brief",
            "--backlog",
            &backlog_path.display().to_string(),
            "--json",
            &brief_path.display().to_string(),
        ],
        dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    brief_path
}

fn make_resolutions(dir: &std::path::Path, data: &Value) -> PathBuf {
    let path = dir.join("resolutions.json");
    write_json(&path, data);
    path
}

fn resolve(dir: &std::path::Path, brief: &PathBuf, resolutions: &PathBuf) -> common::CliOutput {
    run_cli(
        &[
            "resolve",
            "--brief",
            &brief.display().to_string(),
            "--resolutions",
            &resolutions.display().to_string(),
        ],
        dir,
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
fn cli_resolve_prints_the_resolved_record() {
    let dir = temp_dir("resolve-cli-print");
    let brief = make_brief_json(&dir);
    let resolutions = make_resolutions(&dir, &resolutions());
    let out = resolve(&dir, &brief, &resolutions);
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let record = pyjson::parse(&out.stdout).unwrap();
    assert_eq!(
        field(&record, "decision_counts"),
        &value(r#"{"carried": 1, "queued": 0, "resolved": 1}"#)
    );
    assert!(list(&record, "decisions").is_empty());
    let resolved = list(&record, "resolved");
    assert_eq!(text(&resolved[0], "id"), "decision-task-2");
    assert_eq!(text(&resolved[0], "verdict"), "approve");
    assert_eq!(text(&resolved[0], "note"), "ship it tonight");
    assert_eq!(
        field(&record, "counts"),
        &value(r#"{"backlog": 2, "finished": 2, "recorded": 2, "unfinished": 0}"#)
    );
}

#[test]
fn cli_resolve_refuses_an_unknown_decision_with_one_error_line() {
    let dir = temp_dir("resolve-cli-unknown");
    let brief = make_brief_json(&dir);
    let stranger = value(
        r#"{"resolutions": [
            {"decision": "decision-task-9", "verdict": "approve", "note": "who is this"}
        ]}"#,
    );
    let resolutions = make_resolutions(&dir, &stranger);
    let out = resolve(&dir, &brief, &resolutions);
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(out.stderr.trim().lines().count(), 1);
    assert!(out.stderr.contains("decision-task-9"));
    assert!(!out.stderr.contains("Traceback"));
}

#[test]
fn cli_resolve_refuses_a_verdict_that_is_neither_approve_nor_reject() {
    let dir = temp_dir("resolve-cli-verdict");
    let brief = make_brief_json(&dir);
    let shrug = value(
        r#"{"resolutions": [
            {"decision": "decision-task-2", "verdict": "maybe", "note": "undecided"}
        ]}"#,
    );
    let resolutions = make_resolutions(&dir, &shrug);
    let out = resolve(&dir, &brief, &resolutions);
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert!(!out.stderr.contains("Traceback"));
}

#[test]
fn cli_resolve_refuses_a_malformed_resolutions_file() {
    let dir = temp_dir("resolve-cli-malformed");
    let brief = make_brief_json(&dir);
    let bad = dir.join("resolutions.json");
    std::fs::write(&bad, "{not json").unwrap();
    let out = resolve(&dir, &brief, &bad);
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(out.stderr.trim().lines().count(), 1);
    assert!(!out.stderr.contains("Traceback"));
}

#[test]
fn cli_resolve_refuses_a_missing_brief_file() {
    let dir = temp_dir("resolve-cli-missing");
    let resolutions = make_resolutions(&dir, &resolutions());
    let missing = dir.join("nope.json");
    let out = resolve(&dir, &missing, &resolutions);
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert!(!out.stderr.contains("Traceback"));
}

#[test]
fn cli_resolve_over_the_committed_examples_matches_the_committed_resolved_record() {
    let root = root();
    let out = run_cli(
        &[
            "resolve",
            "--brief",
            "examples/brief.json",
            "--resolutions",
            "examples/resolutions.json",
        ],
        &root,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let expected = std::fs::read_to_string(root.join("examples/resolved.json")).unwrap();
    assert_eq!(out.stdout, expected);
}

#[test]
fn cli_resolve_refuses_a_brief_with_a_non_string_decision_id() {
    let dir = temp_dir("resolve-cli-bad-id");
    let brief = dir.join("brief.json");
    write_json(
        &brief,
        &value(
            r#"{"decisions": [
                {"id": "d1", "item": "t1", "question": "q1", "options": ["approve", "reject"]},
                {"id": ["x"], "item": "t2", "question": "q2", "options": ["approve", "reject"]}
            ], "ran": [], "produced": [], "counts": {}, "stock": {}}"#,
        ),
    );
    let resolutions = dir.join("resolutions.json");
    write_json(
        &resolutions,
        &value(r#"{"resolutions": [{"decision": "d1", "verdict": "approve", "note": "n"}]}"#),
    );
    let out = resolve(&dir, &brief, &resolutions);
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(out.stderr.matches('\n').count(), 1);
    assert!(!out.stderr.contains("Traceback"));
}

#[test]
fn cli_resolve_refuses_a_brief_file_with_no_decisions_list() {
    let dir = temp_dir("resolve-cli-no-decisions");
    let not_a_brief = dir.join("brief.json");
    write_json(&not_a_brief, &value(r#"{"tasks": []}"#));
    let resolutions = make_resolutions(&dir, &resolutions());
    let out = resolve(&dir, &not_a_brief, &resolutions);
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert!(!out.stderr.contains("Traceback"));
}
