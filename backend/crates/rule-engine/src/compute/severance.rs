//! §47 经济补偿 — economic compensation (ai/02 §2.4.1).
//!
//! 法源：`law:中华人民共和国劳动合同法/v2012-12-28/§47`.
//! 公式：工龄(按 §47 半年/一年折算) × 月工资(前12月平均)；月工资 > 社平 3 倍时按 3 倍封顶且最长 12 年。

use rust_decimal::dec;
use rust_decimal::Decimal;

use crate::compute::{avg_monthly_wage_pre_12, ceil_to_half_year, region_tag};
use crate::coverage::{ComputedValue, DerivationStep, RuleLawRef, RuleOutcome};
use crate::error::RuleError;
use crate::facts::FactBundle;
use crate::region::RegionParams;

const URN: &str = "law:中华人民共和国劳动合同法/v2012-12-28/§47";

/// Compute the §47 economic compensation. Missing hire/termination dates or wage data return
/// `OutOfScope { Unknown }` (no defaults, C-C-6).
pub fn compute(facts: &FactBundle, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    let (amount, steps) = match severance_amount(facts, region)? {
        Ok(v) => v,
        Err(oos) => return Ok(oos),
    };
    Ok(RuleOutcome::Ok {
        value: ComputedValue::Money(amount),
        coverage_tag: region_tag(region),
        law_refs: vec![RuleLawRef::parse(URN)?],
        derivation: steps,
    })
}

/// Shared kernel reused by `compensation::compute` (§87 = §47 × 2). Returns either the
/// `(amount, derivation)` or an `OutOfScope` outcome to propagate.
pub(crate) fn severance_amount(
    facts: &FactBundle,
    region: &RegionParams,
) -> Result<Result<(Decimal, Vec<DerivationStep>), RuleOutcome>, RuleError> {
    let Some(start) = facts.start_date else {
        return Ok(Err(RuleOutcome::missing_fact(
            "start_date",
            "缺少入职日期，无法计算工龄",
        )));
    };
    let Some(end) = facts.end_date else {
        return Ok(Err(RuleOutcome::missing_fact(
            "end_date",
            "缺少解除日期，无法计算工龄",
        )));
    };
    if facts.wage_records.is_empty() && facts.monthly_wage.is_none() {
        return Ok(Err(RuleOutcome::missing_fact(
            "wage_records",
            "缺少前 12 个月工资明细，无法计算月平均工资",
        )));
    }

    let n = ceil_to_half_year(start, end)?;
    let base_wage = if facts.wage_records.is_empty() {
        facts.monthly_wage.expect("checked above")
    } else {
        avg_monthly_wage_pre_12(&facts.wage_records)?
    };

    let cap_wage =
        region.avg_monthly_wage() * Decimal::from(region.wage.cap_multiplier_for_severance);
    let high_wage = base_wage > cap_wage;
    let actual_wage = if high_wage { cap_wage } else { base_wage };
    // High earners cap at the 3× base AND a maximum of 12 service years.
    let actual_n = if high_wage { n.min(dec!(12)) } else { n };
    let amount = (actual_wage * actual_n).round_dp(2);

    let mut steps = vec![
        DerivationStep::new("工龄折算", format!("({end} − {start}) 按 §47 折算"), n),
        DerivationStep::new(
            "月平均工资",
            "前 12 个月工资合计 ÷ 月数".to_string(),
            base_wage,
        ),
    ];
    if high_wage {
        steps.push(DerivationStep::new(
            "高薪封顶",
            format!("月工资 {base_wage} > 社平 3 倍 {cap_wage}，按 3 倍计、工龄封顶 12 年"),
            cap_wage,
        ));
    }
    steps.push(DerivationStep::new(
        "经济补偿",
        format!("{actual_wage} × {actual_n}"),
        amount,
    ));

    Ok(Ok((amount, steps)))
}
