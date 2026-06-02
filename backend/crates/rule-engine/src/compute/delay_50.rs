//! §85 50% 加付 — malicious-arrears 50% surcharge (ai/02 §2.4.5).
//!
//! 法源：`law:中华人民共和国劳动合同法/v2012-12-28/§85`.
//! 触发条件：劳动监察责令限期支付仍未支付 → 加付 50%。无监察责令记录 → `OutOfScope { Boundary }`。

use rust_decimal::dec;

use crate::compute::region_tag;
use crate::coverage::{
    ComputedValue, CoverageTag, DerivationStep, RuleLawRef, RuleNextAction, RuleOutcome,
};
use crate::error::RuleError;
use crate::facts::FactBundle;
use crate::region::RegionParams;

const URN: &str = "law:中华人民共和国劳动合同法/v2012-12-28/§85";

/// Compute the §85 50% surcharge. Requires an inspection order that remained overdue; otherwise
/// returns `OutOfScope { Boundary }` requesting the third-party inspection record.
pub fn compute(facts: &FactBundle, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    if !facts.has_inspection_order_overdue {
        return Ok(RuleOutcome::OutOfScope {
            coverage_tag: CoverageTag::Boundary,
            reasons: vec!["未提供劳动监察责令限期支付记录，50% 加付不成立".to_string()],
            next_actions: vec![RuleNextAction::CollectEvidence {
                class: data_model::EvidenceCategory::ThirdPartyData,
            }],
        });
    }
    if facts.overdue_wage_amount <= dec!(0) {
        return Ok(RuleOutcome::missing_fact(
            "overdue_wage_amount",
            "缺少拖欠金额，无法计算 50% 加付",
        ));
    }
    let base = facts.overdue_wage_amount;
    let surcharge = (base * dec!(0.5)).round_dp(2);
    Ok(RuleOutcome::Ok {
        value: ComputedValue::Money(surcharge),
        coverage_tag: region_tag(region),
        law_refs: vec![RuleLawRef::parse(URN)?],
        derivation: vec![
            DerivationStep::new("拖欠本金", "劳动监察责令后仍未支付金额".to_string(), base),
            DerivationStep::new("加付赔偿金", format!("{base} × 50%"), surcharge),
        ],
    })
}
