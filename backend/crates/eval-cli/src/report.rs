//! The genuine per-case evaluation report emitted by every gate (ai/05 §5.x).
//!
//! The Rust oracle reports raw actual-vs-expected results; threshold judgement lives in the Python
//! `packages/eval-runner` (`thresholds.py`). `metrics` carries gate-specific aggregate numbers
//! (e.g. PII recall / false-positive rate) that the Python layer asserts against §5.3 constants.

use serde::Serialize;

/// One evaluated case: did the REAL backend output match the law-derived expectation?
#[derive(Debug, Clone, Serialize)]
pub struct CaseResult {
    pub id: String,
    pub pass: bool,
    pub expected: String,
    pub actual: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl CaseResult {
    pub fn new(
        id: impl Into<String>,
        pass: bool,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            pass,
            expected: expected.into(),
            actual: actual.into(),
            note: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// The full gate report. `pass_rate = passed / total`. The Python runner asserts the §5.3 threshold
/// for this gate; the Rust side never decides pass/fail of the gate as a whole.
#[derive(Debug, Clone, Serialize)]
pub struct GateReport {
    pub gate: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rate: f64,
    /// Gate-specific aggregate metrics (e.g. {"recall": 0.97, "fp_rate": 0.0}).
    pub metrics: serde_json::Map<String, serde_json::Value>,
    pub cases: Vec<CaseResult>,
}

impl GateReport {
    pub fn from_cases(gate: impl Into<String>, cases: Vec<CaseResult>) -> Self {
        let total = cases.len();
        let passed = cases.iter().filter(|c| c.pass).count();
        let failed = total - passed;
        let pass_rate = if total == 0 {
            0.0
        } else {
            passed as f64 / total as f64
        };
        Self {
            gate: gate.into(),
            total,
            passed,
            failed,
            pass_rate,
            metrics: serde_json::Map::new(),
            cases,
        }
    }

    pub fn with_metric(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metrics.insert(key.to_string(), value);
        self
    }

    /// One-line human summary for the CLI stdout (no Emoji, ASCII-safe).
    pub fn summary_line(&self) -> String {
        format!(
            "gate={} total={} passed={} failed={} pass_rate={:.4}",
            self.gate, self.total, self.passed, self.failed, self.pass_rate
        )
    }
}
