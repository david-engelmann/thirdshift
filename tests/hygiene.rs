//! Pins for the crate's dependency list as the tool choice
//! ruling restates it, the adopted tool crates and nothing else,
//! and synthetic fixtures only.
//!
//! The marker patterns are assembled from pieces in this file, so
//! the raw text of the file never carries a full marker itself, the
//! same reason the Python original left its own pattern lines out
//! of its scan.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::root;

/// One dependency of the crate as the manifest declares it, the
/// section it stands in and the features it carries.
#[derive(Default)]
struct ManifestScan {
    runtime: BTreeMap<String, Vec<String>>,
    dev_deps: BTreeMap<String, Vec<String>>,
    build: BTreeMap<String, Vec<String>>,
    target: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Bucket {
    Runtime,
    Dev,
    Build,
    Target,
    Other,
}

/// The bucket a section header opens, and the dependency it names
/// when the header itself carries one, the bracketed form.
fn section_bucket(header: &str) -> (Bucket, Option<String>) {
    match header {
        "dependencies" => return (Bucket::Runtime, None),
        "dev-dependencies" => return (Bucket::Dev, None),
        "build-dependencies" => return (Bucket::Build, None),
        _ => {}
    }
    for (prefix, bucket) in [
        ("dependencies.", Bucket::Runtime),
        ("dev-dependencies.", Bucket::Dev),
        ("build-dependencies.", Bucket::Build),
    ] {
        if let Some(name) = header.strip_prefix(prefix) {
            return (bucket, Some(name.to_string()));
        }
    }
    if header.starts_with("target.") && header.contains("dependencies") {
        if let Some((_, name)) = header.rsplit_once("dependencies.") {
            return (Bucket::Target, Some(name.to_string()));
        }
        return (Bucket::Target, None);
    }
    (Bucket::Other, None)
}

/// The features list of one dependency spec, read from the spec
/// itself so a feature the ruling does not name is a finding.
fn features_of(spec: &str) -> Vec<String> {
    let Some(at) = spec.find("features") else {
        return Vec::new();
    };
    let rest = &spec[at..];
    let (Some(open), Some(close)) = (rest.find('['), rest.find(']')) else {
        return Vec::new();
    };
    rest[open + 1..close]
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn scan_manifest(manifest: &str) -> ManifestScan {
    let mut scan = ManifestScan::default();
    let record = |scan: &mut ManifestScan, bucket: Bucket, name: String, features: Vec<String>| {
        let map = match bucket {
            Bucket::Runtime => &mut scan.runtime,
            Bucket::Dev => &mut scan.dev_deps,
            Bucket::Build => &mut scan.build,
            Bucket::Target => &mut scan.target,
            Bucket::Other => return,
        };
        map.insert(name, features);
    };
    let mut bucket = Bucket::Other;
    let mut header_entry: Option<String> = None;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            let header = trimmed.trim_matches(['[', ']']);
            let (next, entry) = section_bucket(header);
            bucket = next;
            header_entry = entry;
            if let Some(name) = &header_entry {
                let name = name.clone();
                record(&mut scan, bucket, name, Vec::new());
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, spec)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        match &header_entry {
            // A key under a bracketed dependency header belongs to
            // that dependency, and only its features are read.
            Some(name) => {
                if key == "features" {
                    let features = features_of(trimmed);
                    let name = name.clone();
                    record(&mut scan, bucket, name, features);
                }
            }
            None => {
                if bucket != Bucket::Other {
                    record(&mut scan, bucket, key.to_string(), features_of(spec));
                }
            }
        }
    }
    scan
}

#[test]
fn the_crate_depends_on_the_adopted_tool_crates_only() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    let scan = scan_manifest(&manifest);
    let names = |map: &BTreeMap<String, Vec<String>>| map.keys().cloned().collect::<Vec<String>>();
    assert_eq!(
        names(&scan.runtime),
        vec![
            "anyhow".to_string(),
            "clap".to_string(),
            "thiserror".to_string()
        ],
        "the runtime dependency list is the adopted tool crates and nothing else"
    );
    assert_eq!(
        scan.runtime.get("clap"),
        Some(&vec!["derive".to_string()]),
        "clap carries the derive feature the ruling names and no other"
    );
    assert_eq!(
        names(&scan.dev_deps),
        vec!["sha2".to_string()],
        "sha2 stands in the dev dependencies, where its one test consumer reads it"
    );
    assert!(scan.build.is_empty(), "no build dependencies");
    assert!(scan.target.is_empty(), "no target dependencies");
    // Positive controls. The scan must be able to fail, so it is
    // shown reading the bracketed header form, distinguishing the
    // sections from each other, and reading a feature the ruling
    // does not name, each of which the assertions above would
    // catch in the real manifest.
    let header_form = "[dependencies.clap]\nversion = \"4\"\nfeatures = [\"derive\", \"env\"]\n";
    let scan = scan_manifest(header_form);
    assert_eq!(
        scan.runtime.get("clap"),
        Some(&vec!["derive".to_string(), "env".to_string()]),
        "the bracketed header form is read with its features"
    );
    let misplaced = "[dependencies]\nsha2 = \"0.10\"\n";
    let scan = scan_manifest(misplaced);
    assert!(
        scan.runtime.contains_key("sha2") && scan.dev_deps.is_empty(),
        "a runtime placement of sha2 reads as runtime, not dev"
    );
    let dev_form = "[dev-dependencies]\nsha2 = \"0.10\"\n";
    let scan = scan_manifest(dev_form);
    assert!(
        scan.dev_deps.contains_key("sha2") && scan.runtime.is_empty(),
        "a dev placement of sha2 reads as dev, not runtime"
    );
}

fn is_local_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
}

fn is_domain_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')
}

fn has_email(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte != b'@' {
            continue;
        }
        let mut local = 0;
        let mut j = at;
        while j > 0 && is_local_char(bytes[j - 1]) {
            j -= 1;
            local += 1;
        }
        if local == 0 {
            continue;
        }
        let mut last_dot: Option<usize> = None;
        let mut k = at + 1;
        while k < bytes.len() && is_domain_char(bytes[k]) {
            if bytes[k] == b'.' {
                last_dot = Some(k);
            }
            k += 1;
        }
        if k == at + 1 {
            continue;
        }
        if let Some(dot) = last_dot {
            if dot == at + 1 {
                continue;
            }
            let suffix = &text[dot + 1..k];
            if suffix.len() >= 2 && suffix.bytes().all(|c| c.is_ascii_alphabetic()) {
                return true;
            }
        }
    }
    false
}

fn has_home_path(text: &str) -> bool {
    let slash_home = ["/ho", "me/"].concat();
    let bytes = text.as_bytes();
    let mut search = text;
    while let Some(at) = search.find(&slash_home) {
        let after = &search[at + slash_home.len()..];
        if after.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return true;
        }
        search = &search[at + slash_home.len()..];
    }
    let windows_tail = ["\\", "Users", "\\"].concat();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte == b':'
            && at > 0
            && bytes[at - 1].is_ascii_alphabetic()
            && text[at + 1..].starts_with(&windows_tail)
        {
            return true;
        }
    }
    false
}

const DOMAIN_SUFFIXES: [&str; 6] = ["com", "net", "org", "io", "dev", "ai"];

fn has_hostname(text: &str) -> bool {
    let prefixes = [
        ["ht", "tp://"].concat(),
        ["ht", "tps://"].concat(),
        ["w", "ww."].concat(),
    ];
    if prefixes.iter().any(|prefix| text.contains(prefix)) {
        return true;
    }
    let bytes = text.as_bytes();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte != b'.' || at == 0 {
            continue;
        }
        if !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'-') {
            continue;
        }
        let after = &text[at + 1..];
        let word: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if !DOMAIN_SUFFIXES.contains(&word.as_str()) {
            continue;
        }
        let tail = &after[word.len()..];
        if !tail.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
            return true;
        }
    }
    false
}

fn markers_in(text: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    if has_email(text) {
        found.push("email address");
    }
    if has_home_path(text) {
        found.push("absolute home path");
    }
    if has_hostname(text) {
        found.push("real looking hostname");
    }
    found
}

#[test]
fn the_marker_checks_flag_planted_markers() {
    // Positive controls. Each check must be able to fail, so each is
    // shown flagging a planted marker built from pieces at runtime.
    let email = ["someone@exa", "mple", ".or", "g"].concat();
    assert!(has_email(&email), "email check missed {}", email);
    let home = ["/ho", "me/synthetic"].concat();
    assert!(has_home_path(&home), "home path check missed {}", home);
    let windows = ["C:", "\\U", "sers\\", "synthetic"].concat();
    assert!(has_home_path(&windows), "windows path check missed");
    let host = ["files", ".or", "g"].concat();
    assert!(has_hostname(&host), "hostname check missed {}", host);
    let url = ["ht", "tp://", "files", ".or", "g"].concat();
    assert!(has_hostname(&url), "url check missed {}", url);
    assert!(!has_email("worker-1 finished the task"));
    assert!(!has_hostname("the brief counts five tasks"));
}

#[test]
fn examples_and_fixtures_carry_no_real_world_markers() {
    let mut targets: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root().join("examples")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                targets.push(path);
            }
        }
    }
    for entry in std::fs::read_dir(root().join("tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|s| s == "rs") {
            targets.push(path);
        }
    }
    let mut offenders = Vec::new();
    for path in targets {
        let text = std::fs::read_to_string(&path).unwrap();
        for marker in markers_in(&text) {
            offenders.push((
                path.strip_prefix(root()).unwrap().display().to_string(),
                marker,
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "non synthetic markers found, {:?}",
        offenders
    );
}
