//! The carry command, TS-42 to TS-47, end to end.

mod common;

use std::path::PathBuf;

use common::{root, run_cli, temp_dir, value, write_json};
use thirdshift::pyjson::{self, Value};

fn carry(dir: &std::path::Path, backlog: &str, brief: &str, out: &str) -> common::CliOutput {
    run_cli(
        &[
            "carry",
            "--backlog",
            backlog,
            "--brief",
            brief,
            "--out",
            out,
        ],
        dir,
    )
}

#[test]
fn cli_carry_over_the_committed_examples_writes_the_committed_carried_backlog() {
    let root = root();
    let dir = temp_dir("carry-cli-examples");
    let out_path = dir.join("carried.json");
    let out = carry(
        &root,
        "examples/backlog.json",
        "examples/windowed-brief.json",
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let expected = std::fs::read_to_string(root.join("examples/carried-backlog.json")).unwrap();
    assert_eq!(
        std::fs::read_to_string(&out_path).unwrap(),
        expected,
        "the written carried backlog is the committed example byte for byte"
    );
    assert_eq!(
        out.stdout, expected,
        "the step prints the same document in the same bytes"
    );
}

#[test]
fn cli_carry_over_the_unwindowed_brief_carries_only_the_failed_task() {
    let root = root();
    let dir = temp_dir("carry-cli-unwindowed");
    let out_path = dir.join("carried.json");
    let out = carry(
        &root,
        "examples/backlog.json",
        "examples/brief.json",
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let carried = pyjson::parse(&out.stdout).unwrap();
    let tasks = carried
        .get("tasks")
        .and_then(Value::as_list)
        .expect("tasks list");
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0],
        value(
            r#"{"id": "task-3", "title": "Rebuild the synthetic index", "kind": "fail", "question": ""}"#
        )
    );
}

#[test]
fn cli_carry_from_the_committed_resolved_record_matches_the_brief_record() {
    let root = root();
    let dir = temp_dir("carry-cli-resolved");
    let plain_path = dir.join("plain.json");
    let resolved_path = dir.join("resolved.json");
    let plain = carry(
        &root,
        "examples/backlog.json",
        "examples/brief.json",
        &plain_path.display().to_string(),
    );
    assert_eq!(plain.code, 0, "stderr {}", plain.stderr);
    let resolved = carry(
        &root,
        "examples/backlog.json",
        "examples/resolved.json",
        &resolved_path.display().to_string(),
    );
    assert_eq!(resolved.code, 0, "stderr {}", resolved.stderr);
    assert_eq!(resolved.stdout, plain.stdout);
    assert_eq!(
        std::fs::read_to_string(&resolved_path).unwrap(),
        std::fs::read_to_string(&plain_path).unwrap()
    );
}

#[test]
fn cli_carry_from_a_record_that_finished_everything_writes_an_empty_tasks_list() {
    let dir = temp_dir("carry-cli-empty");
    let backlog = dir.join("backlog.json");
    write_json(
        &backlog,
        &value(
            r#"{"tasks": [{"id": "task-1", "title": "alpha", "kind": "work", "question": ""}]}"#,
        ),
    );
    let brief_path = dir.join("brief.json");
    let briefed = run_cli(
        &[
            "brief",
            "--backlog",
            &backlog.display().to_string(),
            "--json",
            &brief_path.display().to_string(),
        ],
        &dir,
    );
    assert_eq!(briefed.code, 0, "stderr {}", briefed.stderr);
    let out_path = dir.join("carried.json");
    let out = carry(
        &dir,
        &backlog.display().to_string(),
        &brief_path.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    assert_eq!(
        std::fs::read_to_string(&out_path).unwrap(),
        "{\n  \"tasks\": []\n}\n",
        "the empty handoff is written as a backlog with an empty tasks list"
    );
    let rerun = run_cli(&["run", "--backlog", &out_path.display().to_string()], &dir);
    assert_eq!(rerun.code, 0, "stderr {}", rerun.stderr);
}

fn write_pair(dir: &std::path::Path, backlog: &Value, record: &Value) -> (PathBuf, PathBuf) {
    let backlog_path = dir.join("backlog.json");
    write_json(&backlog_path, backlog);
    let brief_path = dir.join("brief.json");
    write_json(&brief_path, record);
    (backlog_path, brief_path)
}

#[test]
fn cli_carry_refuses_an_unknown_item_with_one_error_line() {
    let dir = temp_dir("carry-cli-unknown");
    let (backlog, brief) = write_pair(
        &dir,
        &value(
            r#"{"tasks": [{"id": "task-1", "title": "alpha", "kind": "work", "question": ""}]}"#,
        ),
        &value(
            r#"{"ran": [
                {"id": "task-1", "status": "finished"},
                {"id": "task-9", "status": "unfinished"}
            ]}"#,
        ),
    );
    let out_path = dir.join("carried.json");
    let out = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - record names item task-9, which the backlog does not carry\n"
    );
    assert!(!out_path.exists(), "a refused carry writes nothing");
}

#[test]
fn cli_carry_refuses_an_unmentioned_item_with_one_error_line() {
    let dir = temp_dir("carry-cli-unmentioned");
    let (backlog, brief) = write_pair(
        &dir,
        &value(
            r#"{"tasks": [
                {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
                {"id": "task-2", "title": "beta", "kind": "work", "question": ""}
            ]}"#,
        ),
        &value(r#"{"ran": [{"id": "task-1", "status": "finished"}]}"#),
    );
    let out_path = dir.join("carried.json");
    let out = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - backlog carries item task-2, which the record does not mention\n"
    );
    assert!(!out_path.exists(), "a refused carry writes nothing");
}

#[test]
fn cli_carry_refuses_a_record_with_no_ran_list() {
    let dir = temp_dir("carry-cli-no-ran");
    let (backlog, brief) = write_pair(
        &dir,
        &value(r#"{"tasks": []}"#),
        &value(r#"{"decisions": []}"#),
    );
    let out_path = dir.join("carried.json");
    let out = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - brief file is not a brief record, it carries no ran list\n"
    );
    assert!(!out_path.exists());
}

#[test]
fn cli_carry_refuses_missing_and_malformed_inputs_with_one_error_line() {
    let dir = temp_dir("carry-cli-bad-inputs");
    let (backlog, brief) = write_pair(&dir, &value(r#"{"tasks": []}"#), &value(r#"{"ran": []}"#));
    let out_path = dir.join("carried.json");
    let missing = dir.join("nope.json");
    let no_backlog = carry(
        &dir,
        &missing.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(no_backlog.code, 2);
    assert_eq!(
        no_backlog.stderr,
        format!(
            "error - backlog file cannot be read, {}\n",
            missing.display()
        )
    );
    let no_brief = carry(
        &dir,
        &backlog.display().to_string(),
        &missing.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(no_brief.code, 2);
    assert_eq!(
        no_brief.stderr,
        format!("error - brief file cannot be read, {}\n", missing.display())
    );
    let bad = dir.join("bad.json");
    std::fs::write(&bad, "{not json").unwrap();
    let malformed = carry(
        &dir,
        &bad.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(malformed.code, 2);
    assert_eq!(malformed.stdout, "");
    assert_eq!(malformed.stderr.trim().lines().count(), 1);
    assert!(!malformed.stderr.contains("Traceback"));
}

#[test]
fn cli_carry_answers_a_failed_write_at_status_1() {
    let dir = temp_dir("carry-cli-bad-out");
    let (backlog, brief) = write_pair(
        &dir,
        &value(
            r#"{"tasks": [{"id": "task-1", "title": "alpha", "kind": "fail", "question": ""}]}"#,
        ),
        &value(r#"{"ran": [{"id": "task-1", "status": "unfinished"}]}"#),
    );
    let out_path = dir.join("no-such-dir").join("carried.json");
    let out = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 1, "stderr {}", out.stderr);
    assert_eq!(out.stdout, "");
    assert_eq!(out.stderr.trim().lines().count(), 1);
    assert!(!out.stderr.contains("Traceback"));
}

#[test]
fn cli_carry_help_names_the_step_and_its_flags() {
    let dir = temp_dir("carry-cli-help");
    let out = run_cli(&["carry", "--help"], &dir);
    assert_eq!(out.code, 0);
    assert!(
        out.stdout.contains("thirdshift carry"),
        "stdout {}",
        out.stdout
    );
    for flag in ["--backlog", "--brief", "--out"] {
        assert!(out.stdout.contains(flag), "help names {flag}");
    }
}

#[test]
fn cli_carry_without_the_output_flag_is_a_usage_error() {
    let dir = temp_dir("carry-cli-usage");
    let out = run_cli(&["carry", "--backlog", "x.json", "--brief", "y.json"], &dir);
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
}
