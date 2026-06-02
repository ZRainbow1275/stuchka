//! 工伤待遇 — work-injury benefit amounts (ai/02 §2.4.6).
//!
//! 法源：`law:工伤保险条例/v2010-12-20/§35` ~ `§37`.
//!
//! - 一次性伤残补助金 = 本人工资 × 月数（1级27月 … 10级7月，全国统一）。
//! - 一次性工伤医疗补助金（解除劳动关系时）= 统筹地区上年职工月平均工资 × 月数（地方规定）。
//! - 一次性伤残就业补助金（解除劳动关系时）= 同上，月数地方规定。
//!
//! 时效与认定阶段见 `deadline::injury_phases`；本节仅算金额。

use rust_decimal::dec;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::coverage::{
    ComputedValue, CoverageTag, DerivationStep, RuleLawRef, RuleNextAction, RuleOutcome,
};
use crate::error::RuleError;
use crate::facts::FactBundle;
use crate::region::RegionParams;

const URN_35: &str = "law:工伤保险条例/v2010-12-20/§35";
const URN_36: &str = "law:工伤保险条例/v2010-12-20/§36";
const URN_37: &str = "law:工伤保险条例/v2010-12-20/§37";

/// Work-injury benefit inputs (ai/02 §2.4.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InjuryInputs {
    /// 伤残等级 1-10 级。
    pub disability_grade: u8,
    /// 本人前 12 月平均工资。
    pub avg_monthly_wage_pre_12: Decimal,
    /// 统筹地区上年职工月平均工资（医疗 / 就业补助基数）。
    pub region_avg_wage: Decimal,
    /// 统筹地区月最低工资（保底基数）。
    pub region_min_wage: Decimal,
    /// 是否解除劳动关系（决定是否计一次性就业 / 医疗补助）。
    pub is_renouncing_labor: bool,
}

/// Compute the work-injury benefit total. Missing `facts.injury` → `OutOfScope { Unknown }`.
/// Make-usable provinces (no subsidy month arrays) report only the lump-sum disability subsidy
/// and route the user to legal aid for the local one-off subsidies (no fabricated values).
pub fn compute(facts: &FactBundle, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    let Some(inj) = &facts.injury else {
        return Ok(RuleOutcome::missing_fact(
            "injury",
            "缺少工伤等级与工资基数，无法计算工伤待遇",
        ));
    };
    if inj.disability_grade < 1 || inj.disability_grade > 10 {
        return Ok(RuleOutcome::OutOfScope {
            coverage_tag: CoverageTag::Unknown,
            reasons: vec![format!(
                "伤残等级 {} 非法（应为 1-10 级）",
                inj.disability_grade
            )],
            next_actions: vec![RuleNextAction::CollectFact {
                field: "disability_grade".to_string(),
            }],
        });
    }
    let idx = (inj.disability_grade - 1) as usize;

    // 1) 一次性伤残补助金 (always payable, national fixed months).
    let lump_months = region
        .injury
        .lump_sum_months_by_grade
        .get(idx)
        .copied()
        .ok_or_else(|| {
            RuleError::Schema(format!("{}: 缺少一次性伤残补助金月数", region.region_code))
        })?;
    let lump_sum = (inj.avg_monthly_wage_pre_12 * Decimal::from(lump_months)).round_dp(2);

    let mut steps = vec![DerivationStep::new(
        "一次性伤残补助金",
        format!(
            "本人工资 {} × {} 月（{} 级）",
            inj.avg_monthly_wage_pre_12, lump_months, inj.disability_grade
        ),
        lump_sum,
    )];
    let mut total = lump_sum;
    let mut law_refs = vec![RuleLawRef::parse(URN_35)?, RuleLawRef::parse(URN_36)?];

    // 2 + 3) 解除劳动关系时的一次性医疗 / 就业补助（地方规定月数）。
    let mut coverage = if region.deep_coverage {
        CoverageTag::Exact
    } else {
        CoverageTag::Approximate
    };

    if inj.is_renouncing_labor {
        let medical = region
            .injury
            .medical_subsidy_months_by_grade
            .get(idx)
            .copied();
        let employment = region
            .injury
            .employment_subsidy_months_by_grade
            .get(idx)
            .copied();
        match (medical, employment) {
            (Some(m), Some(e)) => {
                let medical_amt = (inj.region_avg_wage * Decimal::from(m)).round_dp(2);
                let employment_amt = (inj.region_avg_wage * Decimal::from(e)).round_dp(2);
                steps.push(DerivationStep::new(
                    "一次性工伤医疗补助金",
                    format!("社平 {} × {} 月", inj.region_avg_wage, m),
                    medical_amt,
                ));
                steps.push(DerivationStep::new(
                    "一次性伤残就业补助金",
                    format!("社平 {} × {} 月", inj.region_avg_wage, e),
                    employment_amt,
                ));
                total += medical_amt + employment_amt;
                law_refs.push(RuleLawRef::parse(URN_37)?);
            }
            _ => {
                // make-usable province: month arrays absent → never fabricate; guide to legal aid.
                coverage = CoverageTag::Approximate;
                steps.push(DerivationStep::new(
                    "一次性医疗 / 就业补助",
                    "本省一次性补助月数未做深，请咨询当地法援 12348 / 12333".to_string(),
                    dec!(0),
                ));
            }
        }
    }

    let total = total.round_dp(2);
    steps.push(DerivationStep::new(
        "工伤待遇合计",
        "上述各项加总".to_string(),
        total,
    ));

    Ok(RuleOutcome::Ok {
        value: ComputedValue::Money(total),
        coverage_tag: coverage,
        law_refs,
        derivation: steps,
    })
}
