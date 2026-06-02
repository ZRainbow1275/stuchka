//! D4 physical isolation (`ai/04` §4.9 + `ai/05` §5.10 `hsd_isolation`, A17-A18).
//!
//! Asserts the `hsd` dependency tree contains zero Python bridge / ONNX / Torch / network / async
//! crates. Uses `cargo tree -p hsd` (no network once the lockfile exists), so it runs inside plain
//! `cargo test` alongside the `cargo deny check` gate (`deny.toml`). Scanning only the `hsd`
//! sub-tree is essential — the workspace `api` crate legitimately depends on tokio/axum/reqwest.

use std::process::Command;

/// Crates that must NEVER appear anywhere in the hsd dependency tree (D4).
const BANNED: &[&str] = &[
    // Python bridges
    "pyo3",
    "cpython",
    // ONNX / Torch
    "onnxruntime",
    "ort",
    "tch",
    "torch-sys",
    // candle (R1b deferred this round — must be absent from the R1a tree)
    "candle-core",
    "candle-nn",
    "candle-transformers",
    // network stack
    "reqwest",
    "hyper",
    "ureq",
    "isahc",
    "hf-hub",
    // async runtime / web servers
    "tokio",
    "axum",
    "actix-web",
    "rocket",
    "tonic",
    "tower-http",
    // db
    "sqlx",
];

fn hsd_tree() -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "hsd",
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

/// Extract the crate name from a `cargo tree --prefix none` line (e.g. `regex v1.12.3`).
fn crate_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.split_whitespace().next()
}

#[test]
fn a17_no_banned_crates_in_hsd_tree() {
    let tree = hsd_tree();
    let names: Vec<&str> = tree.lines().filter_map(crate_name).collect();

    // sanity: the scan must see hsd itself + known deps.
    assert!(names.contains(&"hsd"), "scan must see hsd");
    assert!(names.contains(&"regex"), "scan must see regex");
    assert!(names.contains(&"data-model"), "scan must see data-model");

    let mut hits = Vec::new();
    for banned in BANNED {
        if names.contains(banned) {
            hits.push(*banned);
        }
    }
    assert!(
        hits.is_empty(),
        "D4 violated: banned crates in hsd tree: {hits:?}"
    );
}

#[test]
fn a17_no_async_runtime_or_http() {
    let tree = hsd_tree();
    assert!(!tree.contains("\ntokio v"), "no async runtime allowed (D4)");
    assert!(!tree.contains("\nreqwest v"), "no HTTP client allowed (D4)");
    assert!(!tree.contains("\npyo3 v"), "no Python bridge allowed (D4)");
    assert!(
        !tree.contains("\ncandle-core v"),
        "candle deferred to R1b (D4)"
    );
}
