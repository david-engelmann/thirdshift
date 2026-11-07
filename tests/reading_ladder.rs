//! The reading ladder corpus, TS-34. One ladder behind the three
//! file surfaces, each refusing an unreadable path, a file that is
//! not valid UTF-8, and a file that is not valid JSON, on the
//! command line with exit code 2 and one error line, and at the
//! library surface with the error type and message of its own
//! surface. The message sets differ only in the noun each surface
//! names.

mod common;

use std::path::PathBuf;

use common::{list, root, run_cli, temp_dir};
use thirdshift::{
    load_backlog, load_resolutions, read_backlog_file, read_brief_file, read_resolutions_file,
};

const REFUSAL_KINDS: [&str; 3] = [
    "cannot be read, ",
    "is not valid UTF-8, ",
    "is not valid JSON, ",
];

/// The message of a surface refusal. A ladder refusal is a surface
/// error by construction here, and a too deep refusal in its place
/// fails the test that asked for a surface message.
fn surface_error_text<E: std::error::Error>(error: thirdshift::ReadError<E>) -> String {
    match error {
        thirdshift::ReadError::Surface(inner) => inner.to_string(),
        thirdshift::ReadError::TooDeep(message) => {
            panic!("a surface refusal was expected, got, {}", message)
        }
    }
}

fn bad_files(dir: &std::path::Path, stem: &str) -> Vec<PathBuf> {
    let missing = dir.join(format!("{}-missing.json", stem));
    let not_utf8 = dir.join(format!("{}-not-utf8.json", stem));
    std::fs::write(
        &not_utf8,
        b"{\"tasks\": [{\"id\": \"x\", \"title\": \"\xff\xfe\"}]}",
    )
    .unwrap();
    let not_json = dir.join(format!("{}-not-json.json", stem));
    std::fs::write(&not_json, "{not json").unwrap();
    vec![missing, not_utf8, not_json]
}

fn expected_messages(noun: &str, paths: &[PathBuf]) -> Vec<String> {
    REFUSAL_KINDS
        .iter()
        .zip(paths.iter())
        .map(|(kind, path)| format!("{} {}{}", noun, kind, path.display()))
        .collect()
}

#[test]
fn backlog_surface_refusals_on_the_command_line() {
    let dir = temp_dir("ladder-backlog-cli");
    let paths = bad_files(&dir, "backlog");
    for (path, message) in paths.iter().zip(expected_messages("backlog file", &paths)) {
        let out = run_cli(&["run", "--backlog", &path.display().to_string()], &dir);
        assert_eq!(out.code, 2);
        assert_eq!(out.stdout, "");
        assert_eq!(out.stderr, format!("error - {}\n", message));
    }
}

#[test]
fn backlog_surface_refusals_at_the_library_surface() {
    let dir = temp_dir("ladder-backlog-lib");
    let paths = bad_files(&dir, "backlog");
    for (path, message) in paths.iter().zip(expected_messages("backlog file", &paths)) {
        let error = read_backlog_file(path).unwrap_err();
        assert_eq!(surface_error_text(error), message);
        let error = load_backlog(path).unwrap_err();
        assert_eq!(surface_error_text(error), message);
    }
}

#[test]
fn resolutions_surface_refusals_on_the_command_line() {
    let dir = temp_dir("ladder-resolutions-cli");
    let paths = bad_files(&dir, "resolutions");
    let brief = root().join("examples/brief.json");
    for (path, message) in paths
        .iter()
        .zip(expected_messages("resolutions file", &paths))
    {
        let out = run_cli(
            &[
                "resolve",
                "--brief",
                &brief.display().to_string(),
                "--resolutions",
                &path.display().to_string(),
            ],
            &dir,
        );
        assert_eq!(out.code, 2);
        assert_eq!(out.stdout, "");
        assert_eq!(out.stderr, format!("error - {}\n", message));
    }
}

#[test]
fn resolutions_surface_refusals_at_the_library_surface() {
    let dir = temp_dir("ladder-resolutions-lib");
    let paths = bad_files(&dir, "resolutions");
    for (path, message) in paths
        .iter()
        .zip(expected_messages("resolutions file", &paths))
    {
        let error = read_resolutions_file(path).unwrap_err();
        assert_eq!(surface_error_text(error), message);
        let error = load_resolutions(path).unwrap_err();
        assert_eq!(surface_error_text(error), message);
    }
}

#[test]
fn brief_surface_refusals_on_the_command_line() {
    let dir = temp_dir("ladder-brief-cli");
    let paths = bad_files(&dir, "brief");
    let resolutions = root().join("examples/resolutions.json");
    for (path, message) in paths.iter().zip(expected_messages("brief file", &paths)) {
        let out = run_cli(
            &[
                "resolve",
                "--brief",
                &path.display().to_string(),
                "--resolutions",
                &resolutions.display().to_string(),
            ],
            &dir,
        );
        assert_eq!(out.code, 2);
        assert_eq!(out.stdout, "");
        assert_eq!(out.stderr, format!("error - {}\n", message));
    }
}

#[test]
fn brief_surface_refusals_at_the_library_surface() {
    let dir = temp_dir("ladder-brief-lib");
    let paths = bad_files(&dir, "brief");
    for (path, message) in paths.iter().zip(expected_messages("brief file", &paths)) {
        let error = read_brief_file(path).unwrap_err();
        assert_eq!(surface_error_text(error), message);
    }
}

#[test]
fn run_over_the_example_backlog_matches_the_committed_example_brief_json() {
    let root = root();
    let out = run_cli(&["run", "--backlog", "examples/backlog.json"], &root);
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let expected = std::fs::read_to_string(root.join("examples/brief.json")).unwrap();
    assert_eq!(out.stdout, expected);
}

#[test]
fn library_readers_return_the_committed_example_entries() {
    let root = root();
    let entries = load_backlog(&root.join("examples/backlog.json")).unwrap();
    let ids: Vec<String> = entries
        .iter()
        .map(|entry| {
            entry
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    assert_eq!(ids, vec!["task-1", "task-2", "task-3", "task-4", "task-5"]);
    let resolutions = load_resolutions(&root.join("examples/resolutions.json")).unwrap();
    assert_eq!(
        resolutions,
        vec![common::value(
            r#"{"decision": "decision-task-4", "note": "Synthetic build approved for the overnight run", "verdict": "approve"}"#
        )]
    );
    let record = read_brief_file(&root.join("examples/brief.json")).unwrap();
    let decision_ids: Vec<String> = list(&record, "decisions")
        .iter()
        .map(|decision| {
            decision
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    assert_eq!(decision_ids, vec!["decision-task-4"]);
    assert_eq!(
        common::field(&record, "counts"),
        &common::value(r#"{"backlog": 5, "finished": 4, "recorded": 5, "unfinished": 1}"#)
    );
}
