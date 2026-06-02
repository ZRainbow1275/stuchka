//! 上诉时效 15 日 — appeal limitation (ai/06 §6.2).
//!
//! 法源：`law:中华人民共和国民事诉讼法/v2023-09-01/§171`.

use super::anchor::{DeadlineFacts, DeadlineKind};
use super::buffer;
use super::interrupt::effective_remaining;
use crate::coverage::RuleOutcome;
use crate::error::RuleError;
use crate::region::RegionParams;

/// 一审 / 二审上诉时效（默认 15 日，地方参数可覆盖）。
pub fn resolve(
    facts: &DeadlineFacts,
    region: &RegionParams,
    second_instance: bool,
) -> Result<RuleOutcome, RuleError> {
    let days = region.deadline.appeal_days.unwrap_or(15) as i64;
    let state = effective_remaining(days, facts);
    let kind = if second_instance {
        DeadlineKind::AppealSecondInstance
    } else {
        DeadlineKind::AppealFirstInstance
    };
    buffer::apply(state, kind, facts)
}
