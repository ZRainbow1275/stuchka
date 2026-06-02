//! `eval-cli` — the genuine compute oracle for the Stučka evaluation harness (ai/05).
//!
//! Each gate links the REAL production crate and runs a dataset against it, returning a
//! [`report::GateReport`] of per-case actual-vs-expected outcomes. The Python `packages/eval-runner`
//! owns the §5.3 thresholds and decides pass/fail; this crate only reports genuine results so a
//! mis-derived expectation can never silently pass.

pub mod calc;
pub mod common;
pub mod deadline;
pub mod doc;
pub mod law;
pub mod pii;
pub mod report;

use std::path::Path;

use report::GateReport;

/// Crate identity for CI dependency-graph assertions.
pub const CRATE_NAME: &str = "eval-cli";

/// Run a single gate over its dataset. Gate names: `calc | deadline | pii | doc | law`.
pub fn run_gate(gate: &str, dataset: &Path) -> Result<GateReport, String> {
    match gate {
        "calc" => calc::run(dataset),
        "deadline" => deadline::run(dataset),
        "pii" => pii::run(dataset),
        "doc" => doc::run(dataset),
        "law" => law::run(dataset),
        other => Err(format!(
            "unknown gate '{other}' (expected calc|deadline|pii|doc|law)"
        )),
    }
}
