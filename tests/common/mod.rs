//! Shared helpers for the ported test corpus.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use thirdshift::pyjson::{self, Value};

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn value(text: &str) -> Value {
    pyjson::parse(text).expect("test fixture parses")
}

pub fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.get(name).unwrap_or(&Value::Null)
}

pub fn list<'a>(value: &'a Value, name: &str) -> &'a Vec<Value> {
    field(value, name).as_list().expect("field is a list")
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn temp_dir(tag: &str) -> PathBuf {
    let number = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "thirdshift-test-{}-{}-{}",
        std::process::id(),
        tag,
        number
    ));
    std::fs::create_dir_all(&dir).expect("temp dir created");
    dir
}

pub struct CliOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_cli(args: &[&str], cwd: &std::path::Path) -> CliOutput {
    let output = Command::new(env!("CARGO_BIN_EXE_thirdshift"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("binary runs");
    CliOutput {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

pub fn write_json(path: &std::path::Path, data: &Value) {
    let text = pyjson::dumps_indent2(data) + "\n";
    std::fs::write(path, text).expect("file written");
}
