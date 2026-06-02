//! 仲裁时效 — arbitration limitation (ai/06 §6.4): 一般 1 年 + 拖欠工资特殊时效。
//!
//! 法源：`law:中华人民共和国劳动争议调解仲裁法/v2007-12-29/§27`.

use super::anchor::{DeadlineFacts, DeadlineKind};
use super::buffer;
use super::interrupt::effective_remaining;
use crate::coverage::{ComputedValue, CoverageTag, DerivationStep, RuleLawRef, RuleOutcome};
use crate::error::RuleError;

const URN: &str = "law:中华人民共和国劳动争议调解仲裁法/v2007-12-29/§27";

/// 一般仲裁时效 1 年（含中断 / 中止）。
pub fn general(facts: &DeadlineFacts) -> Result<RuleOutcome, RuleError> {
    let state = effective_remaining(365, facts);
    buffer::apply(state, DeadlineKind::ArbitrationGeneral, facts)
}

/// 拖欠工资特殊时效：劳动关系存续期间不受 1 年限制（`Exact` 无倒计时）；
/// 终止的，自终止之日起 1 年内提出。
pub fn wage_arbitration(facts: &DeadlineFacts) -> Result<RuleOutcome, RuleError> {
    if facts.labor_relationship_active {
        let law_refs = vec![RuleLawRef::parse(URN)?];
        let value = super::buffer::DeadlineValue::no_limit_while_active(
            DeadlineKind::ArbitrationWage,
            law_refs.clone(),
        );
        return Ok(RuleOutcome::Ok {
            value: ComputedValue::Deadline(value),
            coverage_tag: CoverageTag::Exact,
            law_refs,
            derivation: vec![DerivationStep::new(
                "拖欠工资特殊时效",
                "劳动关系存续期间拖欠劳动报酬不受 1 年仲裁时效限制；本结果须经人工二次确认"
                    .to_string(),
                rust_decimal::Decimal::ZERO,
            )],
        });
    }
    let ended = match facts.labor_relationship_ended_at {
        Some(d) => d,
        None => {
            return Ok(RuleOutcome::missing_fact(
                "labor_relationship_ended_at",
                "劳动关系已终止但缺少终止日期，无法起算拖欠工资 1 年时效",
            ));
        }
    };
    let state = effective_remaining(365, &facts.with_anchor(ended));
    buffer::apply(state, DeadlineKind::ArbitrationWage, facts)
}
