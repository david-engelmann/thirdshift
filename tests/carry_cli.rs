//! The carry command, TS-42 to TS-47 and TS-54 to TS-55, end to end.

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
fn cli_carry_answers_status_1_for_a_brief_nested_past_the_reader_ceiling() {
    let dir = temp_dir("carry-cli-too-deep");
    let backlog = dir.join("backlog.json");
    std::fs::write(&backlog, "{\"tasks\": []}\n").expect("backlog written");
    let depth = 10_050;
    let mut text = String::from("{\"ran\": [], \"deep\": ");
    text.push_str(&"[".repeat(depth));
    text.push_str(&"]".repeat(depth));
    text.push_str("}\n");
    let brief = dir.join("brief.json");
    std::fs::write(&brief, text).expect("brief written");
    let out_path = dir.join("carried.json");
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
    assert!(!out_path.exists(), "an over-depth brief writes nothing");
}

fn lists(n: usize) -> String {
    "[".repeat(n) + &"]".repeat(n)
}

fn deep_backlog_text(n: usize) -> String {
    format!(
        "{{\"tasks\": [{{\"id\": \"deep\", \"title\": {}, \"kind\": \"work\", \"question\": \"\"}}]}}",
        lists(n)
    )
}

#[test]
fn cli_carry_refuses_a_carried_document_past_the_write_ceiling() {
    // TS-54. The carried document is the object over the tasks list
    // over the entry, three containers above the entry's own chain,
    // so a title chain of 989 nested lists carries at a deepest
    // path of 992, one past the write ceiling of 991. The reader
    // ceiling is 9994, so the entry reads without complaint and
    // only the write refuses, which is what makes the pin bite.
    let dir = temp_dir("carry-cli-write-ceiling");
    let backlog = dir.join("backlog.json");
    std::fs::write(&backlog, deep_backlog_text(989)).expect("backlog written");
    let brief = dir.join("brief.json");
    write_json(
        &brief,
        &value(r#"{"ran": [{"id": "deep", "status": "unfinished"}]}"#),
    );
    let out_path = dir.join("carried.json");
    let out = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 1, "stderr {}", out.stderr);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        format!(
            "error - backlog file is nested too deeply, {}\n",
            out_path.display()
        )
    );
    assert!(
        !out_path.exists(),
        "a carry past the write ceiling writes nothing"
    );
}

#[test]
fn cli_carry_past_the_write_ceiling_leaves_a_standing_out_file_byte_unchanged() {
    // TS-54. The refusal fires before any byte is written, so an
    // out file that already stands keeps its bytes exactly.
    let dir = temp_dir("carry-cli-write-ceiling-standing");
    let backlog = dir.join("backlog.json");
    std::fs::write(&backlog, deep_backlog_text(989)).expect("backlog written");
    let brief = dir.join("brief.json");
    write_json(
        &brief,
        &value(r#"{"ran": [{"id": "deep", "status": "unfinished"}]}"#),
    );
    let out_path = dir.join("carried.json");
    std::fs::write(&out_path, "standing bytes\n").expect("out file written");
    let out = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 1, "stderr {}", out.stderr);
    assert_eq!(out.stdout, "");
    assert_eq!(
        std::fs::read_to_string(&out_path).unwrap(),
        "standing bytes\n",
        "the refused carry leaves the out file byte unchanged"
    );
}

#[test]
fn cli_carry_writes_a_carried_document_at_the_write_ceiling() {
    // TS-54. A title chain of 988 nested lists puts the carried
    // document at a deepest path of exactly 991, the ceiling
    // itself, and the carry writes in the ordinary way.
    let dir = temp_dir("carry-cli-write-at-ceiling");
    let backlog = dir.join("backlog.json");
    std::fs::write(&backlog, deep_backlog_text(988)).expect("backlog written");
    let brief = dir.join("brief.json");
    write_json(
        &brief,
        &value(r#"{"ran": [{"id": "deep", "status": "unfinished"}]}"#),
    );
    let out_path = dir.join("carried.json");
    let out = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &out_path.display().to_string(),
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let written = std::fs::read_to_string(&out_path).unwrap();
    assert_eq!(written, out.stdout);
    let parsed = pyjson::parse(&written).unwrap();
    assert_eq!(pyjson::deepest_path(&parsed), 991);
    let tasks = parsed
        .get("tasks")
        .and_then(Value::as_list)
        .expect("tasks list");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].get("id"), Some(&Value::Str("deep".to_string())));
}

fn naming_pair(dir: &std::path::Path) -> (PathBuf, PathBuf) {
    write_pair(
        dir,
        &value(
            r#"{"tasks": [
                {"id": "task-1", "title": "alpha", "kind": "work", "question": ""},
                {"id": "task-2", "title": "beta", "kind": "fail", "question": ""}
            ]}"#,
        ),
        &value(
            r#"{"ran": [
                {"id": "task-1", "status": "finished"},
                {"id": "task-2", "status": "unfinished"}
            ]}"#,
        ),
    )
}

#[test]
fn cli_carry_with_the_out_path_naming_the_backlog_replaces_it() {
    // TS-55. Both inputs are read whole and the join completes
    // before the write, so naming the backlog file as the out
    // path succeeds and the file then holds the carried backlog.
    let dir = temp_dir("carry-cli-out-is-backlog");
    let (backlog, brief) = naming_pair(&dir);
    let third = dir.join("carried.json");
    let ordinary = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &third.display().to_string(),
    );
    assert_eq!(ordinary.code, 0, "stderr {}", ordinary.stderr);
    let expected = std::fs::read_to_string(&third).unwrap();
    let named = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &backlog.display().to_string(),
    );
    assert_eq!(named.code, 0, "stderr {}", named.stderr);
    assert_eq!(named.stdout, expected);
    assert_eq!(
        std::fs::read_to_string(&backlog).unwrap(),
        expected,
        "the backlog file now holds the carried backlog"
    );
}

#[test]
fn cli_carry_with_the_out_path_naming_the_brief_replaces_it() {
    // TS-55. Naming the brief file as the out path succeeds the
    // same way, and the file then holds the carried backlog in
    // place of the record it held.
    let dir = temp_dir("carry-cli-out-is-brief");
    let (backlog, brief) = naming_pair(&dir);
    let third = dir.join("carried.json");
    let ordinary = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &third.display().to_string(),
    );
    assert_eq!(ordinary.code, 0, "stderr {}", ordinary.stderr);
    let expected = std::fs::read_to_string(&third).unwrap();
    let named = carry(
        &dir,
        &backlog.display().to_string(),
        &brief.display().to_string(),
        &brief.display().to_string(),
    );
    assert_eq!(named.code, 0, "stderr {}", named.stderr);
    assert_eq!(named.stdout, expected);
    assert_eq!(
        std::fs::read_to_string(&brief).unwrap(),
        expected,
        "the brief file now holds the carried backlog"
    );
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
