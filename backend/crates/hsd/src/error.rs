//! HSD error type (`ai/04` §2.2 `HsdError`).
//!
//! `scan()` itself never returns an error (it is pure CPU and the regex layer is always
//! available); these variants surface only from the fallible `try_new` / `NerLayer::try_load`
//! construction paths. NER load failures are swallowed-and-downgraded at the top level
//! (`HsdDetector::try_new` keeps `ner = None`), but `ModelTampered` is propagated so a tampered
//! weight file is a hard error (acceptance A14).

use thiserror::Error;

/// Construction / NER-load errors. `scan()` never returns this (R1a 必死: the regex layer cannot
/// fail at runtime).
#[derive(Debug, Error)]
pub enum HsdError {
    /// A regex rule failed to compile (only possible from a programming error in the rule table).
    #[error("regex compile failed: {0}")]
    RegexCompile(String),
    /// NER weights were requested but not found on disk.
    #[error("NER model weights not found")]
    ModelMissing,
    /// NER weights SHA-256 mismatch — suspected tampering (hard error, A14).
    #[error("NER model weights SHA-256 mismatch (suspected tampering)")]
    ModelTampered,
    /// tokenizer load failed.
    #[error("tokenizer load failed: {0}")]
    Tokenizer(String),
    /// candle inference failed (R1b).
    #[error("NER inference failed: {0}")]
    Inference(String),
    /// Input text exceeds the configured byte budget.
    #[error("input too long (> {0} bytes)")]
    InputTooLong(usize),
}
