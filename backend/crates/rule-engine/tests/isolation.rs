//! INV-01 物理隔离 (ai/02 §2.8 → RE-04 / IS-01 / IS-02).
//!
//! Asserts the rule-engine dependency tree contains zero AI / HTTP / inference / Python crates.
//! Uses `cargo tree -p rule-engine` (no network once the lockfile exists) to resolve the full
//! transitive tree, so the check runs in plain `cargo test` alongside the cargo-deny `deny.toml`
//! gate. Scanning only the `rule-engine` sub-tree (not the whole workspace) is essential — the
//! `api` crate legitimately depends on axum/tokio.

use std::process::Command;

/// Crates that must never appear anywhere in the rule-engine dependency tree (INV-01).
const BANNED: &[&str] = &[
    "tokio",
    "reqwest",
    "hyper",
    "axum",
    "candle-core",
    "candle-nn",
    "candle-transformers",
    "pyo3",
    "onnxruntime",
    "ort",
    "llama-cpp-2",
    "llama-cpp-sys-2",
    "tonic",
    "tower-http",
    "actix-web",
    "rocket",
    "ureq",
    "isahc",
    "sqlx",
];

fn rule_engine_tree() -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "rule-engine",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--target",
            "x86_64-pc-windows-msvc",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 tree")
}

/// Extract the crate name from a `cargo tree --prefix none` line (e.g. `tokio v1.40.0`).
fn crate_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.split_whitespace().next()
}

#[test]
fn is01_no_banned_crates_in_rule_engine_tree() {
    let tree = rule_engine_tree();
    let names: Vec<&str> = tree.lines().filter_map(crate_name).collect();

    // sanity: the scan must see rule-engine itself + a known dependency.
    assert!(names.contains(&"rule-engine"), "scan must see rule-engine");
    assert!(
        names.contains(&"rust_decimal"),
        "scan must see rust_decimal"
    );
    assert!(names.contains(&"data-model"), "scan must see data-model");

    let mut hits = Vec::new();
    for banned in BANNED {
        if names.contains(banned) {
            hits.push(*banned);
        }
    }
    assert!(
        hits.is_empty(),
        "INV-01 violated: banned crates in rule-engine tree: {hits:?}"
    );
}

#[test]
fn is02_no_async_runtime_or_http() {
    let tree = rule_engine_tree();
    assert!(
        !tree.contains("\ntokio v"),
        "no async runtime allowed (INV-01)"
    );
    assert!(
        !tree.contains("\nreqwest v"),
        "no HTTP client allowed (INV-01)"
    );
    assert!(
        !tree.contains("\nsqlx v"),
        "rule-engine must not link sqlx (INV-01)"
    );
}
