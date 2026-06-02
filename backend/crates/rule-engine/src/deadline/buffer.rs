//! 10% buffer + 强制人工二次确认 — INV-08 anchor (ai/06 §6.7).
//!
//! 每个 `Ok` 时效输出 `manual_confirm_required = true` 恒成立；buffered = floor(raw × 0.9)（向下取整
//! 更保守）。严禁任何时效"承诺 / 担保"类措辞（CI 正则 0 命中，见 tests/lint_guards.rs）。

use serde::{Deserialize, Serialize};

use super::anchor::{DeadlineFacts, DeadlineKind};
use super::interrupt::DeadlineState;
use crate::coverage::{ComputedValue, CoverageTag, DerivationStep, RuleLawRef, RuleOutcome};
use crate::error::RuleError;

/// A deadline result with the 10% safety buffer and the INV-08 manual-confirm flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeadlineValue {
    pub kind: DeadlineKind,
    pub raw_remaining_days: i64,
    /// floor(raw × 0.9) — the conservative buffered window.
    pub buffered_remaining_days: i64,
    pub state: DeadlineState,
    /// 恒为 true（INV-08 强制）。
    pub manual_confirm_required: bool,
    pub law_refs: Vec<RuleLawRef>,
}

impl DeadlineValue {
    /// Wage-arrears while the labor relationship is active: not subject to the 1-year clock, so no
    /// countdown. `manual_confirm_required` still holds (INV-08).
    pub fn no_limit_while_active(kind: DeadlineKind, law_refs: Vec<RuleLawRef>) -> Self {
        Self {
            kind,
            raw_remaining_days: i64::MAX,
            buffered_remaining_days: i64::MAX,
            state: DeadlineState::Running {
                remaining_days: i64::MAX,
            },
            manual_confirm_required: true,
            law_refs,
        }
    }
}

/// Raw remaining days for a state (`Expired` → 0).
fn raw_of(state: DeadlineState) -> i64 {
    match state {
        DeadlineState::Running { remaining_days } => remaining_days,
        DeadlineState::Suspended {
            frozen_remaining_days,
        } => frozen_remaining_days,
        DeadlineState::Expired { .. } => 0,
    }
}

/// Apply the 10% buffer with `Exact` coverage (deep-province default).
pub fn apply(
    state: DeadlineState,
    kind: DeadlineKind,
    facts: &DeadlineFacts,
) -> Result<RuleOutcome, RuleError> {
    apply_with_tag(state, kind, facts, CoverageTag::Exact)
}

/// Apply the 10% buffer with an explicit coverage tag (make-usable provinces → `Approximate`).
pub fn apply_with_tag(
    state: DeadlineState,
    kind: DeadlineKind,
    _facts: &DeadlineFacts,
    tag: CoverageTag,
) -> Result<RuleOutcome, RuleError> {
    let raw = raw_of(state);
    // buffered = floor(raw * 0.9). Integer arithmetic floors toward zero for non-negative raw.
    let buffered = if raw <= 0 { 0 } else { (raw * 9) / 10 };
    let law_refs = kind.law_refs()?;
    let value = DeadlineValue {
        kind,
        raw_remaining_days: raw,
        buffered_remaining_days: buffered,
        state,
        manual_confirm_required: true,
        law_refs: law_refs.clone(),
    };
    Ok(RuleOutcome::Ok {
        value: ComputedValue::Deadline(value),
        coverage_tag: tag,
        law_refs,
        derivation: vec![DerivationStep::new(
            "时效剩余",
            format!("原始剩余 {raw} 天，应用 10% 安全余量后建议在 {buffered} 天内完成；本结果须经人工二次确认，系统不作时效承诺"),
            rust_decimal::Decimal::from(buffered),
        )],
    })
}
