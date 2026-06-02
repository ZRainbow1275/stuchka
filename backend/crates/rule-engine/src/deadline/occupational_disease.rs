//! 职业病独立流水线 — occupational-disease pipeline (ai/06 §6.5).
//!
//! 与工伤共用引擎，但起算点为职业病诊断 / 鉴定之日（非事故日），`DeadlineKind::OccupationalDisease`。
//! 法源：`law:中华人民共和国职业病防治法/v2018-12-29/§55`.

use super::anchor::{DeadlineFacts, DeadlineKind};
use super::buffer;
use super::interrupt::effective_remaining;
use crate::coverage::RuleOutcome;
use crate::error::RuleError;

/// 职业病诊断起算 1 年认定时效（`facts.case_occurred_at` 即诊断 / 鉴定之日）。
pub fn recognition(facts: &DeadlineFacts) -> Result<RuleOutcome, RuleError> {
    let state = effective_remaining(365, facts);
    buffer::apply(state, DeadlineKind::OccupationalDisease, facts)
}
