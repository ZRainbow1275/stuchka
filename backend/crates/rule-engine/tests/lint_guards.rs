//! Source-level CI guards (ai/02 §2.8 RE-05, ai/06 §6.10 DL-08).
//!
//! - RE-05: every LawRef is a §0.5 URN; forbidden short-code prefixes have 0 hits in `src/`.
//! - DL-08: `buffer.rs` (and deadline templates) never use "保证 / 一定 / 必然到期" guarantee wording.

use std::fs;
use std::path::{Path, PathBuf};

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn read_all_rs(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(dir).expect("read src dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            read_all_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let content = fs::read_to_string(&path).expect("read rs file");
            out.push((path, content));
        }
    }
}

#[test]
fn re05_no_lawref_short_codes_in_src() {
    let mut files = Vec::new();
    read_all_rs(&src_root(), &mut files);
    let banned_prefixes = ["LCL-", "GD-LCR-", "law_lpct_"];
    for (path, content) in &files {
        for bad in banned_prefixes {
            assert!(
                !content.contains(bad),
                "{}: contains forbidden LawRef short-code {bad} (RE-05, must use §0.5 URN)",
                path.display()
            );
        }
    }
}

#[test]
fn re05_yaml_lawrefs_use_urn_prefix() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    for entry in fs::read_dir(&data).expect("read data dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read yaml");
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("urn:") {
                let value = rest.trim().trim_matches('"');
                assert!(
                    value.starts_with("law:"),
                    "{}: urn value must start with law: — got {value}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn dl08_no_guarantee_wording_in_deadline_src() {
    let deadline_dir = src_root().join("deadline");
    let mut files = Vec::new();
    read_all_rs(&deadline_dir, &mut files);
    // forbid guarantee phrasing about deadlines (the engine never guarantees a limitation outcome).
    let forbidden = ["保证到期", "一定到期", "必然到期"];
    for (path, content) in &files {
        for bad in forbidden {
            assert!(
                !content.contains(bad),
                "{}: contains forbidden guarantee wording {bad} (DL-08)",
                path.display()
            );
        }
    }
}
