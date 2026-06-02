//! §82 未签合同二倍工资 — double wage for an unsigned written contract (ai/02 §2.4.3).
//!
//! 法源：`law:中华人民共和国劳动合同法/v2012-12-28/§82`.
//! 区间：自用工之日起满 1 个月的次日起 → 满 1 年的前一日止（或补签合同前一日），最长 11 个月。
//! 输出为"额外一倍工资"（差额），即 月工资 × 月数。

use rust_decimal::dec;
use rust_decimal::Decimal;

use crate::compute::{months_between_with_partial, region_tag};
use crate::coverage::{ComputedValue, DerivationStep, RuleLawRef, RuleOutcome};
use crate::error::RuleError;
use crate::facts::FactBundle;
use crate::region::RegionParams;

const URN: &str = "law:中华人民共和国劳动合同法/v2012-12-28/§82";

/// Compute the §82 double-wage differential. The differential is the *extra* one-fold wage owed
/// (months × monthly wage), capped at 11 months.
pub fn compute(facts: &FactBundle, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    let Some(start) = facts.start_date else {
        return Ok(RuleOutcome::missing_fact(
            "start_date",
            "缺少用工之日，无法计算二倍工资区间",
        ));
    };
    let monthly = match facts.monthly_wage.or_else(|| {
        if facts.wage_records.is_empty() {
            None
        } else {
            Some(
                facts.wage_records.iter().map(|r| r.gross).sum::<Decimal>()
                    / Decimal::from(facts.wage_records.len() as u64),
            )
        }
    }) {
        Some(w) => w,
        None => {
            return Ok(RuleOutcome::missing_fact(
                "monthly_wage",
                "缺少月工资，无法计算二倍工资",
            ));
        }
    };

    // 起算：用工满 1 个月的次日 (start + 1 month + 1 day).
    let liability_start = start
        .checked_add_months(chrono::Months::new(1))
        .and_then(|d| d.checked_add_days(chrono::Days::new(1)))
        .ok_or(RuleError::Arithmetic)?;
    // 截止：满 1 年的前一日 (start + 1 year - 1 day).
    let one_year_end = start
        .checked_add_months(chrono::Months::new(12))
        .and_then(|d| d.checked_sub_days(chrono::Days::new(1)))
        .ok_or(RuleError::Arithmetic)?;
    // 若已补签合同，截止取补签前一日与 1 年期较早者。
    let liability_end = match facts.contract_signed_at {
        Some(signed) => {
            let signed_prev = signed
                .checked_sub_days(chrono::Days::new(1))
                .unwrap_or(signed);
            one_year_end.min(signed_prev)
        }
        None => one_year_end,
    };

    if liability_end < liability_start {
        // Contract signed within the first month → no double-wage liability window.
        return Ok(RuleOutcome::Ok {
            value: ComputedValue::Money(dec!(0)),
            coverage_tag: region_tag(region),
            law_refs: vec![RuleLawRef::parse(URN)?],
            derivation: vec![DerivationStep::new(
                "二倍工资区间",
                "用工 1 个月内已签订书面合同，无二倍工资责任".to_string(),
                dec!(0),
            )],
        });
    }

    let months = months_between_with_partial(liability_start, liability_end)?.min(dec!(11));
    let amount = (monthly * months).round_dp(2);

    Ok(RuleOutcome::Ok {
        value: ComputedValue::Money(amount),
        coverage_tag: region_tag(region),
        law_refs: vec![RuleLawRef::parse(URN)?],
        derivation: vec![
            DerivationStep::new(
                "二倍工资区间",
                format!("{liability_start} 至 {liability_end}（满 1 月次日起，最长 11 月）"),
                months,
            ),
            DerivationStep::new(
                "二倍工资差额",
                format!("月工资 {monthly} × {months} 个月"),
                amount,
            ),
        ],
    })
}
