//! Smoke tests that run EVERY automated eval gate over the REAL datasets under
//! `packages/eval-runner/datasets` and assert the §5.3 thresholds. This makes `cargo test
//! --workspace` enforce the eval gates too (belt + braces with the Python `packages/eval-runner`):
//! a mis-derived dataset expectation fails here immediately, never silently passing.

use std::path::PathBuf;

fn ds(dir: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/eval-runner/datasets")
        .join(dir)
        .join("manifest.json")
}

fn f64_metric(r: &eval_cli::report::GateReport, key: &str) -> f64 {
    r.metrics
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| panic!("metric {key} missing"))
}

#[test]
fn calc_50_is_100_percent() {
    let r = eval_cli::run_gate("calc", &ds("calc-50")).expect("calc gate runs");
    let fails: Vec<&String> = r.cases.iter().filter(|c| !c.pass).map(|c| &c.id).collect();
    assert_eq!(r.total, 50, "calc-50 must have 50 cases");
    assert_eq!(r.pass_rate, 1.0, "calc-50 must be 100% (M9 错一道不发版); failures: {fails:?}");
}

#[test]
fn deadline_30_is_100_percent() {
    let r = eval_cli::run_gate("deadline", &ds("deadline-30")).expect("deadline gate runs");
    let fails: Vec<&String> = r.cases.iter().filter(|c| !c.pass).map(|c| &c.id).collect();
    assert_eq!(r.total, 30, "deadline-30 must have 30 cases");
    assert_eq!(r.pass_rate, 1.0, "deadline-30 must be 100% (M5 错一道不发版); failures: {fails:?}");
}

#[test]
fn pii_200_recall_and_fp() {
    let r = eval_cli::run_gate("pii", &ds("pii-200")).expect("pii gate runs");
    assert_eq!(r.total, 200, "pii-200 must have 200 cases");
    let recall = f64_metric(&r, "recall");
    let fp_rate = f64_metric(&r, "fp_rate");
    assert!(recall >= 0.95, "PII recall must be >= 95%, got {recall}");
    assert!(fp_rate <= 0.05, "PII false-positive rate must be <= 5%, got {fp_rate}");
}

#[test]
fn doc_20_gb45438_complete() {
    let r = eval_cli::run_gate("doc", &ds("doc-20")).expect("doc gate runs");
    let fails: Vec<&String> = r.cases.iter().filter(|c| !c.pass).map(|c| &c.id).collect();
    assert_eq!(r.total, 20, "doc-20 must have 20 documents");
    assert_eq!(r.pass_rate, 1.0, "GB45438 four-layer completeness must be 100%; failures: {fails:?}");
}

#[test]
fn law_seed_parses_100_percent() {
    let r = eval_cli::run_gate("law", &ds("law-200")).expect("law gate runs");
    let fails: Vec<&String> = r.cases.iter().filter(|c| !c.pass).map(|c| &c.id).collect();
    assert!(r.total >= 50, "law seed must have at least 50 URNs");
    assert_eq!(r.pass_rate, 1.0, "every cited LawRef URN must parse (no hallucinated ids); failures: {fails:?}");
}

#[test]
fn abstention_300_refusal_and_bucket() {
    let r = eval_cli::run_gate("abstention", &ds("abstention-300")).expect("abstention gate runs");
    let fails: Vec<&String> = r.cases.iter().filter(|c| !c.pass).map(|c| &c.id).collect();
    assert_eq!(r.total, 300, "abstention-300 must have 300 cases");
    let bucket_hit = f64_metric(&r, "bucket_hit");
    let refusal_rate = f64_metric(&r, "refusal_rate");
    // INV-08: the production compute_confidence->bucket->compose path must match the spec-derived
    // buckets (>= 90%) and must not over-refuse (Low is a heuristic follow-up, not a refusal).
    assert!(bucket_hit >= 0.90, "bucket_hit must be >= 0.90, got {bucket_hit}; failures: {fails:?}");
    assert!(refusal_rate <= 0.10, "refusal_rate must be <= 0.10, got {refusal_rate}");
}
