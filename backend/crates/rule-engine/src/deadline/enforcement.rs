//! 执行申请时效 2 年 — enforcement-application limitation (ai/06 §6.2).
//!
//! 法源：`law:中华人民共和国民事诉讼法/v2023-09-01/§250`.

use super::anchor::{DeadlineFacts, DeadlineKind};
use super::buffer;
use super::interrupt::effective_remaining;
use crate::coverage::RuleOutcome;
use crate::error::RuleError;
use crate::region::RegionParams;

/// 执行申请时效（默认 2 年，地方参数可覆盖）。
pub fn resolve(facts: &DeadlineFacts, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    let years = region.deadline.enforcement_years.max(1) as i64;
    let state = effective_remaining(years * 365, facts);
    buffer::apply(state, DeadlineKind::Enforcement, facts)
}
