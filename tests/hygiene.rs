//! Pins for TS-10 as the port restates it, a crate that depends on
//! nothing outside the standard library, and synthetic fixtures only.
//!
//! The marker patterns are assembled from pieces in this file, so
//! the raw text of the file never carries a full marker itself, the
//! same reason the Python original left its own pattern lines out
//! of its scan.

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use common::root;

fn dependency_names(manifest: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_dependencies = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed.trim_matches(['[', ']']).ends_with("dependencies");
            continue;
        }
        if in_dependencies {
            if let Some((name, _)) = trimmed.split_once('=') {
                let name = name.trim().trim_matches('"');
                if !name.is_empty() && !name.starts_with('#') {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

#[test]
fn the_crate_depends_on_nothing_outside_the_standard_library() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    let names = dependency_names(&manifest);
    assert!(
        names.is_empty(),
        "dependencies found beyond the standard library, {:?}",
        names
    );
    // The extractor itself is pinned against a manifest that does
    // declare a dependency, so an empty result here is a finding and
    // not a broken scan.
    let sample = "[dependencies]\nserde = \"1\"\n";
    assert_eq!(
        dependency_names(sample),
        BTreeSet::from(["serde".to_string()])
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
        if *byte == b':' && at > 0 && bytes[at - 1].is_ascii_alphabetic() {
            if text[at + 1..].starts_with(&windows_tail) {
                return true;
            }
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
        if path.extension().map_or(false, |s| s == "rs") {
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
