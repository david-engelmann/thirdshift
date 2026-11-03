//! The command line surface corpus.
//!
//! Every case in cli_corpus.json began as a capture from the
//! Python command line of the Round 4 tree, two generation passes
//! agreeing byte for byte. When the command line moved to clap
//! under the maintainer's tool choice ruling, the cases split by
//! their source field. Functional cases, marked oracle, still
//! carry the Python expectation and the binary must match it byte
//! for byte. Presentation cases, marked clap, are the help texts
//! and the malformed invocation refusals, and they carry the
//! forms captured from the clap binary itself, because that
//! presentation is clap's by the ruling. Each case runs the
//! binary in a fresh directory seeded with the committed example
//! files and compares the exit code, both streams, and the full
//! final state of the directory.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::{run_cli, temp_dir};
use thirdshift::pyjson::{self, Value};

const EXPECTED_CASES: usize = 194;

fn string(value: &Value) -> &str {
    value.as_str().expect("corpus text is a string")
}

fn object_entries(value: &Value) -> &Vec<(String, Value)> {
    match value {
        Value::Object(entries) => entries,
        _ => panic!("corpus value is an object"),
    }
}

fn snapshot(dir: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for entry in std::fs::read_dir(dir).expect("case dir readable") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).expect("case file is UTF-8");
            files.insert(name, text);
        }
    }
    files
}

#[test]
fn cli_surface_corpus_matches_the_python_oracle() {
    let corpus = pyjson::parse(include_str!("cli_corpus.json")).expect("corpus parses");
    let setup = object_entries(corpus.get("setup").expect("setup present")).clone();
    let cases = corpus
        .get("cases")
        .expect("cases present")
        .as_list()
        .expect("cases list")
        .clone();
    assert_eq!(cases.len(), EXPECTED_CASES, "corpus case count");
    for case in &cases {
        let name = string(case.get("name").expect("name"));
        let source = string(case.get("source").expect("source"));
        assert!(
            source == "oracle" || source == "clap",
            "case {name}: source is oracle or clap"
        );
        let dir = temp_dir("corpus");
        for (file, text) in &setup {
            std::fs::write(dir.join(file), string(text)).expect("fixture written");
        }
        let argv: Vec<String> = case
            .get("argv")
            .expect("argv")
            .as_list()
            .expect("argv list")
            .iter()
            .map(|item| string(item).to_string())
            .collect();
        let arg_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let out = run_cli(&arg_refs, &dir);
        let expect = case.get("expect").expect("expect");
        let code = match expect.get("code").expect("code") {
            Value::Int(text) => text.parse::<i32>().expect("code is a number"),
            _ => panic!("code is an integer"),
        };
        assert_eq!(out.code, code, "case {name}: exit code");
        assert_eq!(
            out.stdout,
            string(expect.get("stdout").expect("stdout")),
            "case {name}: stdout"
        );
        assert_eq!(
            out.stderr,
            string(expect.get("stderr").expect("stderr")),
            "case {name}: stderr"
        );
        let mut want: BTreeMap<String, String> = BTreeMap::new();
        for (file, text) in object_entries(expect.get("files").expect("files")) {
            want.insert(file.clone(), string(text).to_string());
        }
        assert_eq!(snapshot(&dir), want, "case {name}: final files");
        std::fs::remove_dir_all(&dir).ok();
    }
}
