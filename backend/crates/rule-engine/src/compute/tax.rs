//! 个税 — pre-tax / post-tax switch for severance (ai/02 §2.4.7).
//!
//! 法源：`law:中华人民共和国个人所得税法/v2018-08-31/§4`.
//! 经济补偿在当地社平 3 倍（年）以内免征个税；超出部分单独按综合所得税率表（七级超额累进）。

use rust_decimal::dec;
use rust_decimal::Decimal;

use crate::compute::{comprehensive_income_tax, region_tag};
use crate::coverage::{ComputedValue, DerivationStep, RuleLawRef, RuleOutcome};
use crate::error::RuleError;
use crate::region::RegionParams;

const URN: &str = "law:中华人民共和国个人所得税法/v2018-08-31/§4";

/// Net severance after personal income tax. The portion within 3× the local annual average wage
/// is exempt; the excess is taxed once via the comprehensive-income seven-bracket table.
pub fn after_tax(severance: Decimal, region_avg_monthly_wage: Decimal) -> Decimal {
    let cap = region_avg_monthly_wage * dec!(12) * dec!(3);
    if severance <= cap {
        return severance;
    }
    let taxable = severance - cap;
    let tax = comprehensive_income_tax(taxable);
    (severance - tax).round_dp(2)
}

/// Produce the post-tax severance outcome for the given pre-tax amount and region.
pub fn switch(severance_pre_tax: Decimal, region: &RegionParams) -> Result<RuleOutcome, RuleError> {
    let avg = region.avg_monthly_wage();
    let cap = avg * dec!(12) * dec!(3);
    let net = after_tax(severance_pre_tax, avg);
    let tax = (severance_pre_tax - net).round_dp(2);

    let mut steps = vec![DerivationStep::new(
        "免税额度",
        format!("社平月工资 {avg} × 12 × 3"),
        cap,
    )];
    if severance_pre_tax > cap {
        steps.push(DerivationStep::new(
            "应税部分",
            format!("{severance_pre_tax} − {cap}"),
            severance_pre_tax - cap,
        ));
        steps.push(DerivationStep::new(
            "个人所得税",
            "应税部分按综合所得七级超额累进".to_string(),
            tax,
        ));
    } else {
        steps.push(DerivationStep::new(
            "免征",
            "经济补偿在社平 3 倍以内，免征个税".to_string(),
            dec!(0),
        ));
    }
    steps.push(DerivationStep::new(
        "税后经济补偿",
        format!("{severance_pre_tax} − 个税 {tax}"),
        net,
    ));

    Ok(RuleOutcome::Ok {
        value: ComputedValue::Money(net),
        coverage_tag: region_tag(region),
        law_refs: vec![RuleLawRef::parse(URN)?],
        derivation: steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute::comprehensive_income_tax;

    /// OM-05: the seven-bracket progressive tax is computed entirely in `Decimal`.
    /// Mid-bracket case — 100,000 yuan falls in bracket 2 (10%, quick deduction 2,520):
    /// `100_000 × 0.10 − 2_520 = 7_480`.
    #[test]
    fn tax_mid_bracket_exact_decimal() {
        assert_eq!(comprehensive_income_tax(dec!(100_000)), dec!(7_480.00));
    }

    /// High-bracket case — 500,000 yuan falls in bracket 5 (30%, quick deduction 52,920):
    /// `500_000 × 0.30 − 52_920 = 97_080`.
    #[test]
    fn tax_high_bracket_exact_decimal() {
        assert_eq!(comprehensive_income_tax(dec!(500_000)), dec!(97_080.00));
    }

    /// Top open-ended bracket — 1,000,000 yuan (> 960,000) is taxed at 45%, quick deduction
    /// 181,920: `1_000_000 × 0.45 − 181_920 = 268_080`.
    #[test]
    fn tax_top_bracket_exact_decimal() {
        assert_eq!(comprehensive_income_tax(dec!(1_000_000)), dec!(268_080.00));
    }

    /// First-bracket boundary — exactly 36,000 yuan at 3% (quick deduction 0) = 1,080.
    #[test]
    fn tax_first_bracket_boundary_exact_decimal() {
        assert_eq!(comprehensive_income_tax(dec!(36_000)), dec!(1_080.00));
    }

    /// `after_tax`: severance within the 3× cap is fully exempt; only the excess is taxed via the
    /// Decimal seven-bracket table. Region avg monthly wage 10,000 → cap = 360,000.
    /// Severance 460,000 → taxable 100,000 → tax 7,480 → net 452,520.
    #[test]
    fn after_tax_decimal_excess_over_cap() {
        let net = after_tax(dec!(460_000), dec!(10_000));
        assert_eq!(net, dec!(452_520.00));
    }

    /// OM-05 guard: the production tax money path has no floating-point token in executable code.
    /// Compile-time string scan over the **production** portion (everything before the
    /// `#[cfg(test)]` test module) of `tax.rs` (this file) and `mod.rs` (the bracket table in
    /// `comprehensive_income_tax`). `//` line comments are stripped so doc text mentioning the rule
    /// is not a false positive; any floating-point type in real code fails.
    #[test]
    fn tax_module_money_path_uses_only_decimal() {
        // Built at runtime to avoid the literal appearing in this file's own production scan range.
        let float_tokens = ["f".to_owned() + "64", "f".to_owned() + "32"];
        fn assert_no_float(name: &str, src: &str, tokens: &[String]) {
            // Only scan production code: stop at the test module marker.
            let prod = match src.find("#[cfg(test)]") {
                Some(idx) => &src[..idx],
                None => src,
            };
            for (lineno, line) in prod.lines().enumerate() {
                let code = match line.find("//") {
                    Some(idx) => &line[..idx],
                    None => line,
                };
                for tok in tokens {
                    assert!(
                        !code.contains(tok.as_str()),
                        "unexpected floating-point token in {name} at line {}: {line}",
                        lineno + 1
                    );
                }
            }
        }
        assert_no_float("tax.rs", include_str!("tax.rs"), &float_tokens);
        assert_no_float("mod.rs", include_str!("mod.rs"), &float_tokens);
    }
}
