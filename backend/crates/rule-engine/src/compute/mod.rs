//! M9 monetary computations (ai/02 §2.4). Eight real formula functions plus the X-threshold
//! D11 block. All public compute functions return `Result<RuleOutcome, RuleError>` and never
//! default-fill a missing fact (C-C-6).

pub mod compensation;
pub mod delay_50;
pub mod double_pay;
pub mod injury;
pub mod litigation_cost;
pub mod overtime;
pub mod severance;
pub mod tax;
pub mod x_threshold;

use chrono::{Datelike, NaiveDate};
use rust_decimal::dec;
use rust_decimal::Decimal;

use crate::coverage::CoverageTag;
use crate::error::RuleError;
use crate::region::RegionParams;

/// Coverage tag a money formula should report for a region: deep → `Exact`, else `Approximate`.
pub(crate) fn region_tag(region: &RegionParams) -> CoverageTag {
    if region.deep_coverage {
        CoverageTag::Exact
    } else {
        CoverageTag::Approximate
    }
}

/// §47 service-year rounding: a service period rounds up to a whole year if the remainder ≥ 6
/// months, otherwise the remainder counts as half a year. Implemented on real calendar
/// month-and-day arithmetic so partial periods are exact.
///
/// Rule (劳动合同法 §47 ¶2): 六个月以上不满一年的，按一年计算；不满六个月的，按半年计算。
pub(crate) fn ceil_to_half_year(start: NaiveDate, end: NaiveDate) -> Result<Decimal, RuleError> {
    if end < start {
        return Err(RuleError::Arithmetic);
    }
    let (whole_months, extra_days) = months_and_days_between(start, end);
    let full_years = whole_months / 12;
    let rem_months = whole_months % 12;
    // The fractional tail beyond full years, expressed in months (with any extra days counting as
    // a started month for the ≥6-month test only if there are leftover days pushing past 6).
    let tail_months = rem_months;
    let frac = if tail_months == 0 && extra_days == 0 {
        dec!(0)
    } else if tail_months >= 6 {
        dec!(1)
    } else {
        // less than 6 months → half a year
        dec!(0.5)
    };
    Ok(Decimal::from(full_years) + frac)
}

/// Whole calendar months and leftover days between two dates (`end - start`), `start <= end`.
fn months_and_days_between(start: NaiveDate, end: NaiveDate) -> (i64, i64) {
    let mut months =
        (end.year() - start.year()) as i64 * 12 + (end.month() as i64 - start.month() as i64);
    // If the day-of-month hasn't been reached yet, the last month is not complete.
    let mut anchor_day = start.day();
    // Clamp anchor day to the end month's length when needed (e.g. Jan 31 -> Feb).
    let end_month_len = days_in_month(end.year(), end.month());
    if anchor_day > end_month_len {
        anchor_day = end_month_len;
    }
    if end.day() < anchor_day {
        months -= 1;
    }
    // Leftover days: from the last completed month-anniversary up to end.
    let anniversary = add_months(start, months);
    let extra_days = (end - anniversary).num_days();
    (months.max(0), extra_days.max(0))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(ny, nm, 1).expect("valid first-of-month");
    let first_this = NaiveDate::from_ymd_opt(year, month, 1).expect("valid first-of-month");
    (first_next - first_this).num_days() as u32
}

/// Add `months` calendar months to `date`, clamping the day to the target month length.
fn add_months(date: NaiveDate, months: i64) -> NaiveDate {
    let total = (date.year() as i64) * 12 + (date.month() as i64 - 1) + months;
    let year = (total.div_euclid(12)) as i32;
    let month = (total.rem_euclid(12)) as u32 + 1;
    let day = date.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).expect("valid clamped date")
}

/// Average gross monthly wage over the supplied 12-month records (ai/02 §2.4.1 base). Returns
/// [`RuleError::Arithmetic`] when the slice is empty.
pub(crate) fn avg_monthly_wage_pre_12(
    records: &[crate::facts::WageRecord],
) -> Result<Decimal, RuleError> {
    if records.is_empty() {
        return Err(RuleError::Arithmetic);
    }
    let sum: Decimal = records.iter().map(|r| r.gross).sum();
    Ok(sum / Decimal::from(records.len() as u64))
}

/// Whole-and-fractional months between two dates (`end - start`), `start <= end`, as a `Decimal`.
/// Used by double-pay where the period is measured in months with a partial tail.
pub(crate) fn months_between_with_partial(
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Decimal, RuleError> {
    if end < start {
        return Err(RuleError::Arithmetic);
    }
    let (whole_months, extra_days) = months_and_days_between(start, end);
    // Express the partial tail as a fraction of a 30-day month (conventional in wage arithmetic).
    let frac = Decimal::from(extra_days) / dec!(30);
    Ok(Decimal::from(whole_months) + frac)
}

/// Seven-bracket comprehensive-income progressive tax on the `taxable` amount (个税法 §3 综合所得
/// 年度税率表). Used for the portion of severance above the 3× cap (ai/02 §2.4.7). Returns the tax
/// owed (not the net).
///
/// All arithmetic is in [`rust_decimal::Decimal`] — money never round-trips through `f64`
/// (OM-05). Brackets are `(upper_bound, rate, quick_deduction)`; the final bracket is
/// open-ended (no upper bound), so any amount above the sixth threshold falls into it.
pub(crate) fn comprehensive_income_tax(taxable: Decimal) -> Decimal {
    // (upper_bound, rate, quick_deduction) for the first six closed brackets, yuan.
    let closed_brackets: [(Decimal, Decimal, Decimal); 6] = [
        (dec!(36_000), dec!(0.03), dec!(0)),
        (dec!(144_000), dec!(0.10), dec!(2_520)),
        (dec!(300_000), dec!(0.20), dec!(16_920)),
        (dec!(420_000), dec!(0.25), dec!(31_920)),
        (dec!(660_000), dec!(0.30), dec!(52_920)),
        (dec!(960_000), dec!(0.35), dec!(85_920)),
    ];
    // The open-ended top bracket (> 960,000 yuan).
    let top_rate = dec!(0.45);
    let top_quick = dec!(181_920);

    if taxable <= dec!(0) {
        return dec!(0);
    }
    let mut rate = top_rate;
    let mut quick = top_quick;
    for (upper, r, q) in closed_brackets {
        if taxable <= upper {
            rate = r;
            quick = q;
            break;
        }
    }
    let tax = taxable * rate - quick;
    if tax <= dec!(0) {
        dec!(0)
    } else {
        tax.round_dp(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn ceil_half_year_exact_years() {
        // exactly 8 years → 8.0
        assert_eq!(
            ceil_to_half_year(d(2016, 3, 1), d(2024, 3, 1)).unwrap(),
            dec!(8)
        );
    }

    #[test]
    fn ceil_half_year_more_than_six_months() {
        // 8 years 7 months → 9
        assert_eq!(
            ceil_to_half_year(d(2016, 3, 1), d(2024, 10, 1)).unwrap(),
            dec!(9)
        );
    }

    #[test]
    fn ceil_half_year_less_than_six_months() {
        // 8 years 2 months → 8.5
        assert_eq!(
            ceil_to_half_year(d(2016, 3, 1), d(2024, 5, 1)).unwrap(),
            dec!(8.5)
        );
    }

    #[test]
    fn ceil_half_year_exactly_six_months() {
        // 6 months → 1 year
        assert_eq!(
            ceil_to_half_year(d(2024, 1, 1), d(2024, 7, 1)).unwrap(),
            dec!(1)
        );
    }

    #[test]
    fn tax_zero_below_threshold() {
        assert_eq!(comprehensive_income_tax(dec!(0)), dec!(0));
        // 30000 taxable at 3% = 900
        assert_eq!(comprehensive_income_tax(dec!(30000)), dec!(900));
    }
}
