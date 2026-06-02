//! `RuleError` — the rule-engine error type (ai/02 §2.2, ai/06 §6.x).
//!
//! Every compute / deadline function returns `Result<RuleOutcome, RuleError>`. Missing facts
//! never default-fill (C-C-6): they surface as `RuleOutcome::OutOfScope` (an `Ok` result) rather
//! than `Err`. `RuleError` is reserved for genuinely malformed input (bad URN, region-load
//! failure, schema violation, arithmetic overflow) — conditions a caller cannot recover by
//! supplying one more fact.

use thiserror::Error;

/// Failure modes for the pure rule engine (ai/02 §2.2). `thiserror = "2"` per ai/02 §53.
#[derive(Debug, Error)]
pub enum RuleError {
    /// A required fact is structurally absent at a point where the caller already committed to an
    /// `Ok` path (used by helpers; the public surface prefers `OutOfScope`).
    #[error("缺少必填事实：{0}")]
    MissingFact(&'static str),

    /// Input falls outside the engine's covered legal scope.
    #[error("超出覆盖范围：{0}")]
    OutOfScope(&'static str),

    /// A LawRef URN failed D8 grammar validation (delegated to `data_model::law_ref::parser`).
    #[error("LawRef URN 非法：{0}")]
    BadLawRef(String),

    /// An embedded region YAML could not be loaded / parsed.
    #[error("区域参数加载失败：{0}")]
    RegionLoad(String),

    /// A region YAML parsed but violated a schema invariant (e.g. injury array length != 10).
    #[error("YAML schema 校验失败：{0}")]
    Schema(String),

    /// Arithmetic overflow or division by zero in a formula.
    #[error("计算溢出 / 除零")]
    Arithmetic,
}
