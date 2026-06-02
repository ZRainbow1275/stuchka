//! 胜诉成本对比 — winning-cost comparison (ai/02 §2.4.8).
//!
//! `net_estimate = estimated_recovery − Σfees − time_cost_days × daily_min_wage`，并触发 INV-10
//! 提示（和解金额 < 计算值 × 80% 时前端弹完整免责）。本节产出比值供前端判断。

use rust_decimal::dec;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::compute::region_tag;
use crate::coverage::{ComputedValue, DerivationStep, RuleLawRef, RuleOutcome};
use crate::error::RuleError;
use crate::facts::FactBundle;
use crate::region::RegionParams;

const URN: &str = "law:诉讼费用交纳办法/v2007-04-01/§13";

/// Itemised cost breakdown for the winning-cost comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub arbitration_fee: Decimal,
    pub court_fee_first: Decimal,
    pub court_fee_second: Decimal,
    pub lawyer_fee_estimate: Decimal,
    pub time_cost_days: u32,
    pub estimated_recovery: Decimal,
    pub net_estimate: Decimal,
}

/// Compare expected recovery against fees + time cost. `estimated_recovery` is supplied as the
/// sum of M9 claim amounts (`facts.overdue_wage_amount` proxies the recovery base here; in
/// production the dispatcher passes the aggregated claim total).
pub fn compare(facts: &FactBundle, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    let recovery = facts.overdue_wage_amount;
    if recovery <= dec!(0) {
        return Ok(RuleOutcome::missing_fact(
            "estimated_recovery",
            "缺少预期收益基数，无法对比胜诉成本",
        ));
    }
    // R1: arbitration is free in most regions; court acceptance fee for labor disputes is a flat
    // 10 CNY (诉讼费用交纳办法 §13(5) 劳动争议案件每件交纳 10 元). Lawyer fee defaults to 0.
    let arbitration_fee = dec!(0);
    let court_fee_first = dec!(10);
    let court_fee_second = dec!(10);
    let lawyer_fee = dec!(0);

    // time cost: assume a conservative 180-day cycle; daily min wage = top city min wage / 21.75.
    let time_cost_days: u32 = 180;
    let daily_min = region
        .wage
        .region_min_wage_monthly
        .iter()
        .map(|c| c.value)
        .max()
        .unwrap_or(dec!(0))
        / dec!(21.75);
    let time_cost = (daily_min * Decimal::from(time_cost_days)).round_dp(2);

    let fees = arbitration_fee + court_fee_first + court_fee_second + lawyer_fee;
    let net = (recovery - fees - time_cost).round_dp(2);

    let breakdown = CostBreakdown {
        arbitration_fee,
        court_fee_first,
        court_fee_second,
        lawyer_fee_estimate: lawyer_fee,
        time_cost_days,
        estimated_recovery: recovery,
        net_estimate: net,
    };
    let _ = &breakdown; // breakdown is the structured form; the outcome carries the net figure.

    Ok(RuleOutcome::Ok {
        value: ComputedValue::Money(net),
        coverage_tag: region_tag(region),
        law_refs: vec![RuleLawRef::parse(URN)?],
        derivation: vec![
            DerivationStep::new("预期收益", "M9 各项请求金额加总".to_string(), recovery),
            DerivationStep::new(
                "诉讼 / 仲裁费",
                "劳动争议案件每件 10 元，仲裁多数免收".to_string(),
                fees,
            ),
            DerivationStep::new(
                "时间成本",
                format!("{time_cost_days} 天 × 日最低工资 {}", daily_min.round_dp(2)),
                time_cost,
            ),
            DerivationStep::new("胜诉净收益", "预期收益 − 费用 − 时间成本".to_string(), net),
        ],
    })
}
