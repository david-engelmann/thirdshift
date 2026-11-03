//! Command line edges the surface corpus does not carry.
//!
//! The uncaught failure family of the TS-37 amendment lives here,
//! a failed write and a document nested past a ceiling the Python
//! tool reaches, each answered at status 1 with one error line,
//! and a standard output failure the Python tool discovers only
//! at its exit flush, answered at status 120 with one error line,
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

fn lists_with_scalar(n: usize) -> String {
    "[".repeat(n) + "\"x\"" + &"]".repeat(n)
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
    // The resolver folds the record through recursive copies whose
    // reach is a frame cost, and list shaped depth with an empty
    // list at its foot is the shape that reaches furthest in
    // containers, a path of 497 at the ceiling cost of 994.
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
fn a_brief_with_object_shaped_depth_at_the_ceiling_resolves_and_one_past_it_refuses() {
    // Recorded correction to the window pin this replaces. That
    // pin held a path of 497 containers of object shaped depth
    // open as resolved here and refused by the Python tool, on
    // a reading of the copy ceiling as a container count.
    // Probes against the Python tool show the ceiling is a
    // frame cost, two frames per container on the deepest path
    // and one frame for a scalar at its foot, so object shaped
    // depth with a scalar at its foot reaches a path of 496
    // containers at the ceiling cost and the port refuses one
    // past it exactly as the Python tool does. The window is
    // closed.
    let dir = temp_dir("edges-depth-brief-object");
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
    assert_eq!(out.code, 1);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - brief file is nested too deeply, deep.json\n"
    );
}

#[test]
fn a_brief_with_a_scalar_at_the_foot_of_list_shaped_depth_refuses_at_the_same_cost() {
    // The same ceiling from the other side. A scalar at the
    // foot of list shaped depth costs the copy one frame the
    // empty list at the foot does not, so this shape reaches a
    // path of 496 containers where the empty footed shape
    // pinned earlier in this file reaches 497, and refuses one
    // past it. A container count alone would put the boundary
    // for this shape one document later than the Python tool
    // puts it.
    let dir = temp_dir("edges-depth-brief-scalar-foot");
    write(&dir, "res.json", EMPTY_RESOLUTIONS);
    write(
        &dir,
        "deep.json",
        &format!(
            "{{\"decisions\": [], \"deep\": {}}}",
            lists_with_scalar(495)
        ),
    );
    let out = run_cli(
        &["resolve", "--brief", "deep.json", "--resolutions", "res.json"],
        &dir,
    );
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    write(
        &dir,
        "deep.json",
        &format!(
            "{{\"decisions\": [], \"deep\": {}}}",
            lists_with_scalar(496)
        ),
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
fn a_brief_both_entry_invalid_and_past_the_copy_ceiling_answers_with_the_validation_refusal() {
    // The resolver validates the brief entries ahead of its
    // copies, so a brief that is entry-invalid and past the
    // copy ceiling at once is refused for its entries at
    // status 2, and its mirror, a valid brief equally deep, is
    // refused for its depth at status 1.
    let dir = temp_dir("edges-brief-order-invalid");
    write(&dir, "res.json", EMPTY_RESOLUTIONS);
    write(
        &dir,
        "deep.json",
        &format!(
            "{{\"decisions\": [{{\"id\": \"decision-t\", \"note\": {}}}]}}",
            lists(496)
        ),
    );
    let out = run_cli(
        &["resolve", "--brief", "deep.json", "--resolutions", "res.json"],
        &dir,
    );
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - brief record carries decision decision-t without a string item and question\n"
    );
    write(
        &dir,
        "deep.json",
        &format!(
            "{{\"decisions\": [{{\"id\": \"decision-t\", \"item\": \"task-t\", \"question\": \"q\", \"note\": {}}}]}}",
            lists(496)
        ),
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
fn the_copy_ceilings_and_the_resolution_validation_answer_in_the_resolvers_order() {
    // The decisions copy stands ahead of the resolution
    // validation and the record copy behind it. Depth carried
    // inside a decisions entry past the ceiling therefore
    // answers at status 1 ahead of a defective resolution,
    // depth inside an entry at the ceiling leaves the
    // resolution refusal standing at status 2, and depth
    // carried outside the decisions is reached only by the
    // record copy, behind the validation, so it answers with
    // the resolution refusal at status 2 as well.
    let dir = temp_dir("edges-brief-order-copies");
    write(
        &dir,
        "res.json",
        "{\"resolutions\": [{\"decision\": \"decision-nope\", \"verdict\": \"approve\", \"note\": \"x\"}]}",
    );
    let entry_deep = |n: usize| {
        format!(
            "{{\"decisions\": [{{\"id\": \"decision-a\", \"item\": \"task-a\", \"question\": \"q\", \"deep\": {}}}]}}",
            lists(n)
        )
    };
    write(&dir, "deep.json", &entry_deep(495));
    let out = run_cli(
        &["resolve", "--brief", "deep.json", "--resolutions", "res.json"],
        &dir,
    );
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - resolution names decision decision-nope, which the brief does not carry\n"
    );
    write(&dir, "deep.json", &entry_deep(496));
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
    write(
        &dir,
        "deep.json",
        &format!("{{\"decisions\": [], \"deep\": {}}}", lists(497)),
    );
    let out = run_cli(
        &["resolve", "--brief", "deep.json", "--resolutions", "res.json"],
        &dir,
    );
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error - resolution names decision decision-nope, which the brief does not carry\n"
    );
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

fn run_with_full_stdout(args: &[&str], dir: &std::path::Path) -> (i32, String) {
    let full = std::fs::File::options()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full opens");
    let out = Command::new(env!("CARGO_BIN_EXE_thirdshift"))
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .expect("binary runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stderr).expect("stderr is UTF-8"),
    )
}

#[test]
fn a_stdout_failure_found_only_at_the_exit_flush_exits_120() {
    // Recorded correction to the earlier pin, which held every
    // stdout failure at status 1. The Python tool holds its
    // printed output in a buffer of 8192 bytes, so the small
    // output of an empty backlog is never written through
    // during the run, and the failure surfaces only when the
    // buffer is flushed as the tool exits, where the Python
    // tool reports an exception ignored and exits with status
    // 120. The port answers at status 120 with its one error
    // line in place of that report.
    let dir = temp_dir("edges-full");
    write(&dir, "backlog.json", EMPTY_BACKLOG);
    let (code, stderr) = run_with_full_stdout(&["run", "--backlog", "backlog.json"], &dir);
    assert_eq!(code, 120);
    assert_eq!(
        stderr,
        "error - stdout: No space left on device (os error 28)\n"
    );
}

#[test]
fn a_stdout_failure_under_a_large_output_exits_1() {
    // An output past the buffer size is written through as it
    // is printed, so the failure surfaces during the run and
    // escapes at status 1, the uncaught shape.
    let dir = temp_dir("edges-full-large");
    let tasks: Vec<String> = (0..55)
        .map(|i| {
            format!(
                "{{\"id\": \"t{}\", \"title\": \"x\", \"kind\": \"work\", \"question\": \"\"}}",
                i
            )
        })
        .collect();
    write(
        &dir,
        "backlog.json",
        &format!("{{\"tasks\": [{}]}}", tasks.join(", ")),
    );
    let (code, stderr) = run_with_full_stdout(&["run", "--backlog", "backlog.json"], &dir);
    assert_eq!(code, 1);
    assert_eq!(
        stderr,
        "error - stdout: No space left on device (os error 28)\n"
    );
}

#[test]
fn the_stdout_shapes_divide_at_the_buffer_size() {
    // The boundary itself, pinned on both sides. One task with
    // a padded title sizes the printed record exactly. The
    // unpadded length is measured off the output itself, and
    // the padding lands the record at 8191 bytes, which exits
    // 120, and at 8192 bytes, which exits 1.
    let dir = temp_dir("edges-full-boundary");
    let backlog = |pad: usize| {
        format!(
            "{{\"tasks\": [{{\"id\": \"t\", \"title\": \"x{}\", \"kind\": \"work\", \"question\": \"\"}}]}}",
            "p".repeat(pad)
        )
    };
    write(&dir, "backlog.json", &backlog(0));
    let out = run_cli(&["run", "--backlog", "backlog.json"], &dir);
    assert_eq!(out.code, 0, "stderr {}", out.stderr);
    let base = out.stdout.len();
    write(&dir, "backlog.json", &backlog(8191 - base));
    let (code, _) = run_with_full_stdout(&["run", "--backlog", "backlog.json"], &dir);
    assert_eq!(code, 120);
    write(&dir, "backlog.json", &backlog(8192 - base));
    let (code, _) = run_with_full_stdout(&["run", "--backlog", "backlog.json"], &dir);
    assert_eq!(code, 1);
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
