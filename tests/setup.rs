//! The setup invariants, re-expressed as Rust tests under TS-41.

mod common;

use common::root;

const CANONICAL_PREFIX_SHA256: &str =
    "63d2f04005dc8eb111d3efca1be55c0911f9ad46bc7cf3c74d9f9908f4a86f11";
const EXPECTED_PREFIX: &str = "TS";
const TEXT_SUFFIXES: [&str; 9] = [
    "md", "yml", "yaml", "py", "toml", "txt", "json", "jsonl", "rs",
];
const SKIP_PARTS: [&str; 8] = [
    ".git",
    "target",
    ".pytest_cache",
    "__pycache__",
    ".venv",
    "venv",
    "node_modules",
    ".tox",
];

#[test]
fn contract_stub_obligations_exist() {
    let text = std::fs::read_to_string(root().join("CONTRACT.md")).unwrap();
    let mut found: Vec<(u32, String)> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(EXPECTED_PREFIX) else {
            continue;
        };
        let Some(rest) = rest.strip_prefix("-S") else {
            continue;
        };
        let Some((number, _)) = rest.split_once('.') else {
            continue;
        };
        if number.len() == 1 {
            if let Ok(number) = number.parse::<u32>() {
                found.push((number, line.to_string()));
            }
        }
    }
    assert!(
        found.len() >= 5,
        "expected at least 5 stub obligations, found {}",
        found.len()
    );
    let mut numbers: Vec<u32> = found.iter().map(|(number, _)| *number).collect();
    numbers.sort_unstable();
    assert_eq!(
        numbers,
        vec![1, 2, 3, 4, 5],
        "stub obligations must be exactly S1 to S5"
    );
    for (_, line) in &found {
        let words = line.split_whitespace().count();
        assert!(words >= 20, "obligation too thin at {} words", words);
    }
}

#[test]
fn agents_canonical_block_present() {
    let text = std::fs::read_to_string(root().join("AGENTS.md")).unwrap();
    assert!(text.contains("repo annex below this line"));
    let prefix = text.split("## Repo annex").next().unwrap();
    let digest = sha256_hex(prefix.as_bytes());
    assert_eq!(
        digest, CANONICAL_PREFIX_SHA256,
        "canonical block above the annex line has drifted"
    );
    let annex = text.split("## Repo annex").nth(1).unwrap();
    assert!(annex.contains("CONTRACT.md"));
}

#[test]
fn readme_status_reports_rounds() {
    let text = std::fs::read_to_string(root().join("README.md")).unwrap();
    let counts = closed_round_counts(&text);
    assert_eq!(
        counts,
        vec!["4".to_string()],
        "README status must report exactly one closed round count"
    );
}

/// Every match of the pinned form, a closed rounds count whose
/// number is not followed by another digit.
fn closed_round_counts(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let marker = "Closed rounds ";
    let mut search = text;
    while let Some(at) = search.find(marker) {
        let after = &search[at + marker.len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        let rest = &after[digits.len()..];
        if !digits.is_empty() && rest.starts_with(" of 100") {
            let tail = &rest[" of 100".len()..];
            if !tail.starts_with(|c: char| c.is_ascii_digit()) {
                found.push(digits);
            }
        }
        search = &search[at + marker.len()..];
    }
    found
}

#[test]
fn no_em_or_en_dash_in_text_files() {
    let em = "\u{2014}".as_bytes();
    let en = "\u{2013}".as_bytes();
    let mut bad = Vec::new();
    let mut stack = vec![root()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root()).unwrap().to_path_buf();
            if relative
                .components()
                .any(|part| SKIP_PARTS.contains(&part.as_os_str().to_str().unwrap_or("")))
            {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let suffix = path
                .extension()
                .map(|s| s.to_str().unwrap_or(""))
                .unwrap_or("");
            if !TEXT_SUFFIXES.contains(&suffix) {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            if contains_bytes(&bytes, em) || contains_bytes(&bytes, en) {
                bad.push(relative);
            }
        }
    }
    assert!(bad.is_empty(), "em or en dash found in {:?}", bad);
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// A small SHA-256, here so the setup test needs no dependency. The
/// AGENTS pin below is its own check, and the known vector test pins
/// the implementation itself.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
        0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
        0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
        0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for block in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) =
            (state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
    state.iter().map(|word| format!("{:08x}", word)).collect()
}

#[test]
fn sha256_matches_a_known_vector() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
