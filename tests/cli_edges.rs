//! Command line edges the surface corpus does not carry.
//!
//! The uncaught failure family of the TS-37 amendment lives here,
//! a failed write and a document nested past a ceiling the Python
//! tool reaches, each answered at status 1 with one error line,
//! and the argument bytes the corpus format cannot hold, a path
//! and a data value that are not UTF-8. Depths are generated in
//! code because the boundary documents run to thousands of
//! containers. Every boundary is pinned on both sides, the deepest
//! document the Python tool carries and the first it refuses.

mod common;

use std::path::PathBuf;
use std::process::{Command, Stdio};

use common::{root, run_cli, temp_dir};

const EMPTY_BACKLOG: &str = "{\"tasks\": []}";
const EMPTY_RESOLUTIONS: &str = "{\"resolutions\": []}";

fn lists(n: usize) -> String {
    "[".repeat(n) + &"]".repeat(n)
}

fn objects(n: usize) -> String {
    let mut text = "1".to_string();
    for _ in 0..n {
        text = format!("{{\"a\": {}}}", text);
    }
    text
}

fn write(dir: &std::path::Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("fixture written");
    path
}

#[test]
fn a_backlog_at_the_reader_ceiling_runs_and_one_past_it_refuses_at_status_1() {
    let dir = temp_dir("edges-depth-backlog");
    write(&dir, "backlog.json", EMPTY_BACKLOG);
    // A wrapped backlog, the root object plus 9993 lists is a path
    // of 9994 containers, the deepest the Python reader carries.
    write(
        &dir,
        "deep.json",
        &format!("{{\"tasks\": [], \"deep\": {}}}", lists(9993)),
    );
    let out = run_cli(&["run", "--backlog", "deep.json"], &dir);
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    write(
        &dir,
        "deep.json",
        &format!("{{\"tasks\": [], \"deep\": {}}}", lists(9994)),
    );
    let out = run_cli(&["run", "--backlog", "deep.json"], &dir);
    assert_eq!(out.code, 1);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - backlog file is nested too deeply, deep.json\n"
    );
}

#[test]
fn a_resolutions_file_at_the_reader_ceiling_resolves_and_one_past_it_refuses() {
    let dir = temp_dir("edges-depth-resolutions");
    write(&dir, "brief.json", &root_brief());
    write(
        &dir,
        "deep.json",
        &format!("{{\"resolutions\": [], \"deep\": {}}}", lists(9993)),
    );
    let out = run_cli(
        &["resolve", "--brief", "brief.json", "--resolutions", "deep.json"],
        &dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    write(
        &dir,
        "deep.json",
        &format!("{{\"resolutions\": [], \"deep\": {}}}", lists(9994)),
    );
    let out = run_cli(
        &["resolve", "--brief", "brief.json", "--resolutions", "deep.json"],
        &dir,
    );
    assert_eq!(out.code, 1);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - resolutions file is nested too deeply, deep.json\n"
    );
}

fn root_brief() -> String {
    std::fs::read_to_string(root().join("examples/brief.json"))
        .expect("example brief readable")
}

#[test]
fn a_brief_at_the_record_ceiling_resolves_and_one_past_it_refuses() {
    let dir = temp_dir("edges-depth-brief");
    write(&dir, "res.json", EMPTY_RESOLUTIONS);
    // The resolver folds the record through a recursive copy that
    // reaches a path of 497 containers on list shaped depth.
    write(
        &dir,
        "deep.json",
        &format!("{{\"decisions\": [], \"deep\": {}}}", lists(496)),
    );
    let out = run_cli(
        &["resolve", "--brief", "deep.json", "--resolutions", "res.json"],
        &dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    write(
        &dir,
        "deep.json",
        &format!("{{\"decisions\": [], \"deep\": {}}}", lists(497)),
    );
    let out = run_cli(
        &["resolve", "--brief", "deep.json", "--resolutions", "res.json"],
        &dir,
    );
    assert_eq!(out.code, 1);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - brief file is nested too deeply, deep.json\n"
    );
}

#[test]
fn a_brief_with_object_shaped_depth_at_the_recorded_window_still_resolves() {
    // The recorded window of the TS-37 amendment. Object shaped
    // depth in the Python tool reaches a path of 496 containers,
    // one less than list shaped depth, and the port holds the
    // list boundary, so a brief whose object shaped depth stands
    // at a path of 497 containers resolves here where the Python
    // tool refuses it. This pin holds that window open on purpose
    // so it cannot drift silently in either direction.
    let dir = temp_dir("edges-depth-brief-window");
    write(&dir, "res.json", EMPTY_RESOLUTIONS);
    write(
        &dir,
        "deep.json",
        &format!("{{\"decisions\": [], \"deep\": {}}}", objects(495)),
    );
    let out = run_cli(
        &["resolve", "--brief", "deep.json", "--resolutions", "res.json"],
        &dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    write(
        &dir,
        "deep.json",
        &format!("{{\"decisions\": [], \"deep\": {}}}", objects(496)),
    );
    let out = run_cli(
        &["resolve", "--brief", "deep.json", "--resolutions", "res.json"],
        &dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
}

#[test]
fn an_enqueue_at_the_written_ceiling_writes_and_one_past_it_leaves_the_file() {
    let dir = temp_dir("edges-depth-enqueue");
    // The written document is the root object, the tasks list,
    // the entry, and the entry's deep title, so a title nested
    // 988 deep writes a path of 991 containers, the deepest the
    // Python tool's indented writer carries.
    let at_ceiling = format!(
        "{{\"tasks\": [{{\"id\": \"x\", \"title\": {}, \"kind\": \"work\", \"question\": \"\"}}]}}",
        lists(988)
    );
    write(&dir, "deep.json", &at_ceiling);
    let out = run_cli(
        &["enqueue", "--file", "deep.json", "--id", "y", "--title", "t"],
        &dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let past_ceiling = format!(
        "{{\"tasks\": [{{\"id\": \"x\", \"title\": {}, \"kind\": \"work\", \"question\": \"\"}}]}}",
        lists(989)
    );
    write(&dir, "deep.json", &past_ceiling);
    let out = run_cli(
        &["enqueue", "--file", "deep.json", "--id", "y", "--title", "t"],
        &dir,
    );
    assert_eq!(out.code, 1);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - backlog file is nested too deeply, deep.json\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("deep.json")).unwrap(),
        past_ceiling,
        "the refused enqueue leaves the backlog file unwritten"
    );
}

#[test]
fn failed_writes_exit_1_with_one_error_line() {
    let dir = temp_dir("edges-writes");
    write(&dir, "backlog.json", EMPTY_BACKLOG);
    std::fs::create_dir(dir.join("somedir")).expect("directory made");

    let out = run_cli(
        &["run", "--backlog", "backlog.json", "--result", "nodir/out.json"],
        &dir,
    );
    assert_eq!(out.code, 1);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - nodir/out.json: No such file or directory (os error 2)\n"
    );

    let out = run_cli(
        &["run", "--backlog", "backlog.json", "--result", "somedir"],
        &dir,
    );
    assert_eq!(out.code, 1);
    assert_eq!(
        out.stderr,
        "error - somedir: Is a directory (os error 21)\n"
    );

    let out = run_cli(
        &["brief", "--backlog", "backlog.json", "--md", "nodir/brief.md"],
        &dir,
    );
    assert_eq!(out.code, 1);
    assert_eq!(
        out.stderr,
        "error - nodir/brief.md: No such file or directory (os error 2)\n"
    );

    // The brief writes its Markdown before its JSON, as the Python
    // tool does, so a JSON write failure leaves the Markdown file
    // standing and still exits 1.
    let out = run_cli(
        &[
            "brief",
            "--backlog",
            "backlog.json",
            "--md",
            "ok.md",
            "--json",
            "nodir/brief.json",
        ],
        &dir,
    );
    assert_eq!(out.code, 1);
    assert_eq!(
        out.stderr,
        "error - nodir/brief.json: No such file or directory (os error 2)\n"
    );
    assert!(dir.join("ok.md").is_file(), "the Markdown write stands");

    let out = run_cli(
        &["enqueue", "--file", "nodir/e.json", "--id", "i", "--title", "t"],
        &dir,
    );
    assert_eq!(out.code, 1);
    assert_eq!(
        out.stderr,
        "error - nodir/e.json: No such file or directory (os error 2)\n"
    );
}

#[test]
fn a_stdout_that_cannot_take_the_bytes_exits_1() {
    let dir = temp_dir("edges-full");
    write(&dir, "backlog.json", EMPTY_BACKLOG);
    let full = std::fs::File::options()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full opens");
    let out = Command::new(env!("CARGO_BIN_EXE_thirdshift"))
        .args(["run", "--backlog", "backlog.json"])
        .current_dir(&dir)
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .expect("binary runs");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(out.stderr).expect("stderr is UTF-8"),
        "error - stdout: No space left on device (os error 28)\n"
    );
}

#[cfg(unix)]
#[test]
fn a_backlog_path_that_is_not_utf8_runs() {
    use std::os::unix::ffi::OsStrExt as _;
    let dir = temp_dir("edges-nonutf8");
    let raw = std::ffi::OsStr::from_bytes(b"backlog-\xff.json");
    std::fs::write(dir.join(raw), EMPTY_BACKLOG).expect("fixture written");
    let out = Command::new(env!("CARGO_BIN_EXE_thirdshift"))
        .args([
            std::ffi::OsStr::new("run"),
            std::ffi::OsStr::new("--backlog"),
            raw,
        ])
        .current_dir(&dir)
        .output()
        .expect("binary runs");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).expect("stdout is UTF-8");
    assert!(stdout.contains("\"backlog\": 0"), "stdout {}", stdout);
}

#[cfg(unix)]
#[test]
fn a_data_value_that_is_not_utf8_reads_as_the_replacement_character() {
    use std::os::unix::ffi::OsStrExt as _;
    // The recorded reading for text a Rust string cannot hold the
    // way a Python string holds it. The Python tool stores the id
    // with its surrogate escape, the port stores the replacement
    // character, both under the surrogate exception family.
    let dir = temp_dir("edges-nonutf8-value");
    let raw_id = std::ffi::OsStr::from_bytes(b"i\xff");
    let out = Command::new(env!("CARGO_BIN_EXE_thirdshift"))
        .args([
            std::ffi::OsStr::new("enqueue"),
            std::ffi::OsStr::new("--file"),
            std::ffi::OsStr::new("e.json"),
            std::ffi::OsStr::new("--id"),
            raw_id,
            std::ffi::OsStr::new("--title"),
            std::ffi::OsStr::new("t"),
        ])
        .current_dir(&dir)
        .output()
        .expect("binary runs");
    assert_eq!(out.status.code(), Some(0));
    let written = std::fs::read_to_string(dir.join("e.json")).expect("file written");
    assert!(
        written.contains("\\ufffd"),
        "the stored id carries the replacement character, {}",
        written
    );
}
