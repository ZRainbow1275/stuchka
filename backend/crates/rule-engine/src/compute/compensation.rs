//! §87 违法解除赔偿金 — illegal-termination damages (ai/02 §2.4.2).
//!
//! 法源：`law:中华人民共和国劳动合同法/v2012-12-28/§87`.
//! 公式：经济补偿 × 2，封顶规则同 §47。

use rust_decimal::dec;

use crate::compute::{region_tag, severance};
use crate::coverage::{ComputedValue, DerivationStep, RuleLawRef, RuleOutcome};
use crate::error::RuleError;
use crate::facts::FactBundle;
use crate::region::RegionParams;

const URN_47: &str = "law:中华人民共和国劳动合同法/v2012-12-28/§47";
const URN_87: &str = "law:中华人民共和国劳动合同法/v2012-12-28/§87";

/// Compute the §87 illegal-termination damages = §47 economic compensation × 2.
pub fn compute(facts: &FactBundle, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    let (severance_amount, mut steps) = match severance::severance_amount(facts, region)? {
        Ok(v) => v,
        Err(oos) => return Ok(oos),
    };
    let amount = (severance_amount * dec!(2)).round_dp(2);
    steps.push(DerivationStep::new(
        "违法解除赔偿",
        format!("经济补偿 {severance_amount} × 2"),
        amount,
    ));
    Ok(RuleOutcome::Ok {
        value: ComputedValue::Money(amount),
        coverage_tag: region_tag(region),
        law_refs: vec![RuleLawRef::parse(URN_47)?, RuleLawRef::parse(URN_87)?],
        derivation: steps,
    })
}
