//! 劳动监察投诉时效 2 年 — labor-inspection complaint limitation (ai/06 §6.2).
//!
//! 法源：`law:劳动保障监察条例/v2004-12-01/§20`.

use super::anchor::{DeadlineFacts, DeadlineKind};
use super::buffer;
use super::interrupt::effective_remaining;
use crate::error::RuleError;
use crate::region::RegionParams;

/// 监察投诉时效（默认 2 年，地方参数可覆盖）。
pub fn resolve(
    facts: &DeadlineFacts,
    region: &RegionParams,
) -> Result<crate::coverage::RuleOutcome, RuleError> {
    let years = region.deadline.inspection_years.max(1) as i64;
    let state = effective_remaining(years * 365, facts);
    buffer::apply(state, DeadlineKind::Inspection, facts)
}
