//! §44 加班费 — overtime pay (ai/02 §2.4.4).
//!
//! 法源：`law:中华人民共和国劳动法/v2018-12-29/§44`.
//! 时薪 = 月工资 / 21.75 / 8；工作日 1.5×、休息日 2.0×、法定节假日 3.0×。

use rust_decimal::dec;
use rust_decimal::Decimal;

use crate::compute::region_tag;
use crate::coverage::{ComputedValue, DerivationStep, RuleLawRef, RuleOutcome};
use crate::error::RuleError;
use crate::facts::FactBundle;
use crate::region::RegionParams;

const URN: &str = "law:中华人民共和国劳动法/v2018-12-29/§44";

/// Compute the §44 overtime pay across weekday / weekend / holiday hours.
pub fn compute(facts: &FactBundle, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    let Some(monthly) = facts.monthly_wage else {
        return Ok(RuleOutcome::missing_fact(
            "monthly_wage",
            "缺少月工资，无法折算加班费时薪",
        ));
    };

    let divisor = region.overtime.hourly_divisor;
    if divisor.is_zero() {
        return Err(RuleError::Arithmetic);
    }
    let hourly = (monthly / divisor / dec!(8)).round_dp(4);

    let weekday = (facts.weekday_ot_hours * hourly * region.overtime.weekday_rate).round_dp(2);
    let weekend = (facts.weekend_ot_hours * hourly * region.overtime.weekend_rate).round_dp(2);
    let holiday = (facts.holiday_ot_hours * hourly * region.overtime.holiday_rate).round_dp(2);
    let amount: Decimal = weekday + weekend + holiday;

    Ok(RuleOutcome::Ok {
        value: ComputedValue::Money(amount),
        coverage_tag: region_tag(region),
        law_refs: vec![RuleLawRef::parse(URN)?],
        derivation: vec![
            DerivationStep::new("时薪", format!("{monthly} ÷ {divisor} ÷ 8"), hourly),
            DerivationStep::new(
                "工作日加班",
                format!(
                    "{} × {} × {}",
                    facts.weekday_ot_hours, hourly, region.overtime.weekday_rate
                ),
                weekday,
            ),
            DerivationStep::new(
                "休息日加班",
                format!(
                    "{} × {} × {}",
                    facts.weekend_ot_hours, hourly, region.overtime.weekend_rate
                ),
                weekend,
            ),
            DerivationStep::new(
                "法定节假日加班",
                format!(
                    "{} × {} × {}",
                    facts.holiday_ot_hours, hourly, region.overtime.holiday_rate
                ),
                holiday,
            ),
            DerivationStep::new(
                "加班费合计",
                "工作日 + 休息日 + 法定节假日".to_string(),
                amount,
            ),
        ],
    })
}
