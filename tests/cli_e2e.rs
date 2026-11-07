//! End to end, the CLI over the committed example backlog and
//! over a built one.

mod common;

use common::{field, list, root, run_cli, temp_dir};
use thirdshift::pyjson;

#[test]
fn cli_run_over_example_backlog_records_all_five_tasks() {
    let root = root();
    let out = run_cli(&["run", "--backlog", "examples/backlog.json"], &root);
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let record = pyjson::parse(&out.stdout).unwrap();
    assert_eq!(
        field(&record, "counts"),
        &common::value(r#"{"backlog": 5, "finished": 4, "recorded": 5, "unfinished": 1}"#)
    );
    assert_eq!(list(&record, "decisions").len(), 1);
}

#[test]
fn cli_run_reads_a_repeated_key_under_its_last_value() {
    // TS-56 on the run surface. The entry names its kind twice
    // and the run reads the last value the document carried. With
    // work first and fail second the item is recorded unfinished,
    // where reading the first value would record it finished, and
    // the mirror entry finishes.
    let dir = temp_dir("e2e-repeated-key");
    let backlog = dir.join("backlog.json");
    std::fs::write(
        &backlog,
        "{\"tasks\": [{\"id\": \"task-1\", \"title\": \"alpha\", \"kind\": \"work\", \"kind\": \"fail\", \"question\": \"\"}]}\n",
    )
    .unwrap();
    let out = run_cli(&["run", "--backlog", &backlog.display().to_string()], &dir);
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let record = pyjson::parse(&out.stdout).unwrap();
    let ran = list(&record, "ran");
    assert_eq!(ran.len(), 1);
    assert_eq!(field(&ran[0], "status"), &common::value("\"unfinished\""));
    std::fs::write(
        &backlog,
        "{\"tasks\": [{\"id\": \"task-1\", \"title\": \"alpha\", \"kind\": \"fail\", \"kind\": \"work\", \"question\": \"\"}]}\n",
    )
    .unwrap();
    let out = run_cli(&["run", "--backlog", &backlog.display().to_string()], &dir);
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let record = pyjson::parse(&out.stdout).unwrap();
    let ran = list(&record, "ran");
    assert_eq!(ran.len(), 1);
    assert_eq!(field(&ran[0], "status"), &common::value("\"finished\""));
}

#[test]
fn cli_brief_over_example_backlog_matches_the_committed_example_brief() {
    let root = root();
    let dir = temp_dir("e2e-brief");
    let md_path = dir.join("brief.md");
    let json_path = dir.join("brief.json");
    let out = run_cli(
        &[
            "brief",
            "--backlog",
            "examples/backlog.json",
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
        std::fs::read_to_string(root.join("examples/brief.md")).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(&json_path).unwrap(),
        std::fs::read_to_string(root.join("examples/brief.json")).unwrap()
    );
}

#[test]
fn cli_enqueue_run_brief_end_to_end() {
    let dir = temp_dir("e2e-flow");
    let backlog = dir.join("backlog.json");
    let first = run_cli(
        &[
            "enqueue",
            "--file",
            &backlog.display().to_string(),
            "--id",
            "task-1",
            "--title",
            "alpha",
        ],
        &dir,
    );
    assert_eq!(first.code, 0, "stderr {}", first.stderr);
    let second = run_cli(
        &[
            "enqueue",
            "--file",
            &backlog.display().to_string(),
            "--id",
            "task-2",
            "--title",
            "gate",
            "--kind",
            "decision",
            "--question",
            "Ship the synthetic gate",
        ],
        &dir,
    );
    assert_eq!(second.code, 0, "stderr {}", second.stderr);
    let md_path = dir.join("brief.md");
    let json_path = dir.join("brief.json");
    let brief = run_cli(
        &[
            "brief",
            "--backlog",
            &backlog.display().to_string(),
            "--md",
            &md_path.display().to_string(),
            "--json",
            &json_path.display().to_string(),
        ],
        &dir,
    );
    assert_eq!(brief.code, 0, "stderr {}", brief.stderr);
    let data = pyjson::parse(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
    assert_eq!(
        field(field(&data, "counts"), "recorded"),
        &thirdshift::Value::uint(2)
    );
    let decisions = list(&data, "decisions");
    assert_eq!(
        decisions[0].get("question").and_then(|q| q.as_str()),
        Some("Ship the synthetic gate")
    );
    assert!(
        std::fs::read_to_string(&md_path)
            .unwrap()
            .contains("task-1")
    );
}

#[test]
fn cli_enqueue_rejects_a_duplicate_id() {
    let dir = temp_dir("e2e-duplicate");
    let backlog = dir.join("backlog.json");
    let first = run_cli(
        &[
            "enqueue",
            "--file",
            &backlog.display().to_string(),
            "--id",
            "task-1",
            "--title",
            "alpha",
        ],
        &dir,
    );
    assert_eq!(first.code, 0);
    let again = run_cli(
        &[
            "enqueue",
            "--file",
            &backlog.display().to_string(),
            "--id",
            "task-1",
            "--title",
            "alpha again",
        ],
        &dir,
    );
    assert_eq!(again.code, 2);
}

#[test]
fn cli_rejects_invalid_json_with_a_clean_error() {
    let dir = temp_dir("e2e-bad-json");
    let backlog = dir.join("backlog.json");
    std::fs::write(&backlog, "{not json").unwrap();
    let out = run_cli(
        &["brief", "--backlog", &backlog.display().to_string()],
        &dir,
    );
    assert_eq!(out.code, 2);
    assert!(!out.stderr.trim().is_empty());
    assert!(!out.stderr.contains("Traceback"));
}

#[test]
fn cli_rejects_a_backlog_object_without_a_tasks_list() {
    let dir = temp_dir("e2e-bad-shape");
    let backlog = dir.join("backlog.json");
    common::write_json(&backlog, &common::value(r#"{"items": []}"#));
    let out = run_cli(&["run", "--backlog", &backlog.display().to_string()], &dir);
    assert_eq!(out.code, 2);
    assert!(!out.stderr.trim().is_empty());
    assert!(!out.stderr.contains("Traceback"));
}

#[test]
fn cli_invalid_utf8_backlog_exits_2_without_traceback() {
    let root = root();
    let dir = temp_dir("e2e-bad-utf8");
    let backlog = dir.join("bad.json");
    std::fs::write(
        &backlog,
        b"{\"tasks\": [{\"id\": \"x\", \"title\": \"\xff\xfe\"}]}",
    )
    .unwrap();
    let out = run_cli(&["run", "--backlog", &backlog.display().to_string()], &root);
    assert_eq!(out.code, 2);
    assert!(!out.stderr.contains("Traceback"));
}
