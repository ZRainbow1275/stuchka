//! M9 计算单元测试 (ai/02 §2.8 → RE-01/03/06/07/08). >= 30 cases.

use chrono::NaiveDate;
use rust_decimal::dec;
use rust_decimal::Decimal;

use rule_engine::compute::injury::InjuryInputs;
use rule_engine::compute::{
    compensation, delay_50, double_pay, injury, litigation_cost, overtime, severance, tax,
    x_threshold,
};
use rule_engine::coverage::{ComputedValue, CoverageTag, RuleNextAction, RuleOutcome};
use rule_engine::facts::{FactBundle, WageRecord};
use rule_engine::region::RegionTable;
use rule_engine::{RuleEngine, RuleIntent, RuleRequest};

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn money(o: &RuleOutcome) -> Decimal {
    match o {
        RuleOutcome::Ok {
            value: ComputedValue::Money(m),
            ..
        } => *m,
        other => panic!("expected money Ok, got {other:?}"),
    }
}

fn flat_wage_records(start: NaiveDate, monthly: Decimal) -> Vec<WageRecord> {
    (0..12)
        .map(|i| {
            let m = start
                .checked_add_months(chrono::Months::new(i))
                .unwrap_or(start);
            WageRecord::new(m, monthly)
        })
        .collect()
}

fn gd() -> rule_engine::region::RegionParams {
    RegionTable::load_embedded()
        .unwrap()
        .get("44")
        .unwrap()
        .clone()
}

// ───────────────────────────── severance (RE-07) ─────────────────────────────

#[test]
fn re07_01_severance_normal_8y_8000() {
    // 广东深圳 8 年工龄，月薪 8000 → 64000.
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2016, 3, 1)),
        end_date: Some(d(2024, 3, 1)),
        wage_records: flat_wage_records(d(2016, 3, 1), dec!(8000)),
        ..Default::default()
    };
    let o = severance::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(64000));
    assert_eq!(o.coverage_tag(), CoverageTag::Exact);
}

#[test]
fn re07_02_severance_partial_half_year() {
    // 3 年 2 个月 → 3.5 年 × 6000 = 21000.
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2021, 1, 1)),
        end_date: Some(d(2024, 3, 1)),
        monthly_wage: Some(dec!(6000)),
        wage_records: flat_wage_records(d(2021, 1, 1), dec!(6000)),
        ..Default::default()
    };
    let o = severance::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(21000));
}

#[test]
fn re07_03_severance_partial_full_year() {
    // 3 年 7 个月 → 4 年 × 6000 = 24000.
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2021, 1, 1)),
        end_date: Some(d(2024, 8, 1)),
        wage_records: flat_wage_records(d(2021, 1, 1), dec!(6000)),
        ..Default::default()
    };
    let o = severance::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(24000));
}

#[test]
fn re07_04_severance_cap_3x() {
    // 月薪 60000 > 社平 3 倍 (10923*3=32769)，工龄 15 年封顶 12 年 → 32769 × 12 = 393228.
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2009, 3, 1)),
        end_date: Some(d(2024, 3, 1)),
        wage_records: flat_wage_records(d(2009, 3, 1), dec!(60000)),
        ..Default::default()
    };
    let o = severance::compute(&facts, &region).unwrap();
    let cap = dec!(10923) * dec!(3);
    assert_eq!(money(&o), cap * dec!(12));
}

#[test]
fn re07_05_severance_cap_3x_short_service_not_capped_years() {
    // 月薪 60000 > 3 倍，但工龄仅 5 年（< 12），年限不封顶 → 32769 × 5.
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2019, 3, 1)),
        end_date: Some(d(2024, 3, 1)),
        wage_records: flat_wage_records(d(2019, 3, 1), dec!(60000)),
        ..Default::default()
    };
    let o = severance::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(32769) * dec!(5));
}

#[test]
fn re06_06_severance_missing_start_date_abstains() {
    // RE-06: 缺 start_date → OutOfScope{Unknown} + CollectFact，禁默认值。
    let region = gd();
    let facts = FactBundle {
        end_date: Some(d(2024, 3, 1)),
        wage_records: flat_wage_records(d(2016, 3, 1), dec!(8000)),
        ..Default::default()
    };
    let o = severance::compute(&facts, &region).unwrap();
    match o {
        RuleOutcome::OutOfScope {
            coverage_tag,
            next_actions,
            ..
        } => {
            assert_eq!(coverage_tag, CoverageTag::Unknown);
            assert!(matches!(
                next_actions.first(),
                Some(RuleNextAction::CollectFact { field }) if field == "start_date"
            ));
        }
        _ => panic!("expected OutOfScope"),
    }
}

#[test]
fn re06_07_severance_missing_wage_abstains() {
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2016, 3, 1)),
        end_date: Some(d(2024, 3, 1)),
        ..Default::default()
    };
    let o = severance::compute(&facts, &region).unwrap();
    assert!(!o.is_ok());
}

#[test]
fn re02_08_severance_approximate_for_make_usable_province() {
    // 26 省 make-usable → Approximate.
    let table = RegionTable::load_embedded().unwrap();
    let henan = table.get("41").unwrap();
    let facts = FactBundle {
        start_date: Some(d(2016, 3, 1)),
        end_date: Some(d(2024, 3, 1)),
        wage_records: flat_wage_records(d(2016, 3, 1), dec!(5000)),
        ..Default::default()
    };
    let o = severance::compute(&facts, henan).unwrap();
    assert_eq!(o.coverage_tag(), CoverageTag::Approximate);
    assert_eq!(money(&o), dec!(40000));
}

// ───────────────────────────── compensation (RE-08) ─────────────────────────────

#[test]
fn re08_09_compensation_is_double_severance() {
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2016, 3, 1)),
        end_date: Some(d(2024, 3, 1)),
        wage_records: flat_wage_records(d(2016, 3, 1), dec!(8000)),
        ..Default::default()
    };
    let o = compensation::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(128000));
}

#[test]
fn re08_10_compensation_cap_same_as_severance() {
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2009, 3, 1)),
        end_date: Some(d(2024, 3, 1)),
        wage_records: flat_wage_records(d(2009, 3, 1), dec!(60000)),
        ..Default::default()
    };
    let o = compensation::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(10923) * dec!(3) * dec!(12) * dec!(2));
}

#[test]
fn re08_11_compensation_missing_fact_abstains() {
    let region = gd();
    let facts = FactBundle::default();
    assert!(!compensation::compute(&facts, &region).unwrap().is_ok());
}

// ───────────────────────────── double pay (RE-08) ─────────────────────────────

#[test]
fn re08_12_double_pay_full_11_months() {
    // 用工 2024-01-01，从未补签 → 接近 11 个月封顶。
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2024, 1, 1)),
        monthly_wage: Some(dec!(6000)),
        ..Default::default()
    };
    let o = double_pay::compute(&facts, &region).unwrap();
    // months in [10, 11], amount in [60000, 66000].
    let amt = money(&o);
    assert!(amt <= dec!(66000), "double pay <= 11 months: {amt}");
    assert!(amt >= dec!(60000), "double pay near full window: {amt}");
}

#[test]
fn re08_13_double_pay_signed_early_zero() {
    // 用工 1 个月内补签 → 0.
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2024, 1, 1)),
        monthly_wage: Some(dec!(6000)),
        contract_signed_at: Some(d(2024, 1, 20)),
        ..Default::default()
    };
    let o = double_pay::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(0));
}

#[test]
fn re08_14_double_pay_signed_midway() {
    // 用工 2024-01-01，补签 2024-05-01 → 约 3 个月差额。
    let region = gd();
    let facts = FactBundle {
        start_date: Some(d(2024, 1, 1)),
        monthly_wage: Some(dec!(6000)),
        contract_signed_at: Some(d(2024, 5, 1)),
        ..Default::default()
    };
    let o = double_pay::compute(&facts, &region).unwrap();
    let amt = money(&o);
    assert!(
        amt > dec!(0) && amt < dec!(24000),
        "midway double pay: {amt}"
    );
}

#[test]
fn re08_15_double_pay_missing_start_abstains() {
    let region = gd();
    let facts = FactBundle {
        monthly_wage: Some(dec!(6000)),
        ..Default::default()
    };
    assert!(!double_pay::compute(&facts, &region).unwrap().is_ok());
}

// ───────────────────────────── overtime (RE-08) ─────────────────────────────

#[test]
fn re08_16_overtime_weekday_1_5x() {
    // 月薪 8700 → 时薪 8700/21.75/8 = 50. 10 工作日小时 × 1.5 × 50 = 750.
    let region = gd();
    let facts = FactBundle {
        monthly_wage: Some(dec!(8700)),
        weekday_ot_hours: dec!(10),
        ..Default::default()
    };
    let o = overtime::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(750));
}

#[test]
fn re08_17_overtime_weekend_2x() {
    // 8 休息日小时 × 2.0 × 50 = 800.
    let region = gd();
    let facts = FactBundle {
        monthly_wage: Some(dec!(8700)),
        weekend_ot_hours: dec!(8),
        ..Default::default()
    };
    let o = overtime::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(800));
}

#[test]
fn re08_18_overtime_holiday_3x() {
    // 8 法定节假日小时 × 3.0 × 50 = 1200.
    let region = gd();
    let facts = FactBundle {
        monthly_wage: Some(dec!(8700)),
        holiday_ot_hours: dec!(8),
        ..Default::default()
    };
    let o = overtime::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(1200));
}

#[test]
fn re08_19_overtime_combined() {
    // 750 + 800 + 1200 = 2750.
    let region = gd();
    let facts = FactBundle {
        monthly_wage: Some(dec!(8700)),
        weekday_ot_hours: dec!(10),
        weekend_ot_hours: dec!(8),
        holiday_ot_hours: dec!(8),
        ..Default::default()
    };
    let o = overtime::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(2750));
}

#[test]
fn re06_20_overtime_missing_wage_abstains() {
    let region = gd();
    let facts = FactBundle {
        weekday_ot_hours: dec!(10),
        ..Default::default()
    };
    assert!(!overtime::compute(&facts, &region).unwrap().is_ok());
}

// ───────────────────────────── delay 50% ─────────────────────────────

#[test]
fn re_21_delay50_with_inspection_order() {
    let region = gd();
    let facts = FactBundle {
        has_inspection_order_overdue: true,
        overdue_wage_amount: dec!(20000),
        ..Default::default()
    };
    let o = delay_50::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(10000));
}

#[test]
fn re_22_delay50_without_order_is_boundary() {
    let region = gd();
    let facts = FactBundle {
        overdue_wage_amount: dec!(20000),
        ..Default::default()
    };
    let o = delay_50::compute(&facts, &region).unwrap();
    assert_eq!(o.coverage_tag(), CoverageTag::Boundary);
    assert!(!o.is_ok());
}

// ───────────────────────────── injury ─────────────────────────────

#[test]
fn re_23_injury_grade_7_not_renouncing() {
    // 7 级一次性伤残补助金 = 本人工资 × 13 月。本人工资 6000 → 78000.
    let region = gd();
    let facts = FactBundle {
        injury: Some(InjuryInputs {
            disability_grade: 7,
            avg_monthly_wage_pre_12: dec!(6000),
            region_avg_wage: dec!(10923),
            region_min_wage: dec!(2360),
            is_renouncing_labor: false,
        }),
        ..Default::default()
    };
    let o = injury::compute(&facts, &region).unwrap();
    assert_eq!(money(&o), dec!(78000));
}

#[test]
fn re_24_injury_grade_7_renouncing_adds_subsidies() {
    // 解除劳动关系：加医疗(4月) + 就业(25月)，均以社平 10923 计。
    let region = gd();
    let facts = FactBundle {
        injury: Some(InjuryInputs {
            disability_grade: 7,
            avg_monthly_wage_pre_12: dec!(6000),
            region_avg_wage: dec!(10923),
            region_min_wage: dec!(2360),
            is_renouncing_labor: true,
        }),
        ..Default::default()
    };
    let o = injury::compute(&facts, &region).unwrap();
    let expected = dec!(78000) + dec!(10923) * dec!(4) + dec!(10923) * dec!(25);
    assert_eq!(money(&o), expected);
}

#[test]
fn re_25_injury_invalid_grade_abstains() {
    let region = gd();
    let facts = FactBundle {
        injury: Some(InjuryInputs {
            disability_grade: 11,
            avg_monthly_wage_pre_12: dec!(6000),
            region_avg_wage: dec!(10923),
            region_min_wage: dec!(2360),
            is_renouncing_labor: false,
        }),
        ..Default::default()
    };
    assert!(!injury::compute(&facts, &region).unwrap().is_ok());
}

#[test]
fn re_26_injury_missing_input_abstains() {
    let region = gd();
    assert!(!injury::compute(&FactBundle::default(), &region)
        .unwrap()
        .is_ok());
}

#[test]
fn re_27_injury_make_usable_renouncing_stays_approximate() {
    // make-usable 省份缺月数数组 → Approximate，仅出一次性伤残补助金，不臆造补助。
    let table = RegionTable::load_embedded().unwrap();
    let henan = table.get("41").unwrap();
    let facts = FactBundle {
        injury: Some(InjuryInputs {
            disability_grade: 7,
            avg_monthly_wage_pre_12: dec!(6000),
            region_avg_wage: dec!(6395),
            region_min_wage: dec!(2100),
            is_renouncing_labor: true,
        }),
        ..Default::default()
    };
    let o = injury::compute(&facts, henan).unwrap();
    assert_eq!(o.coverage_tag(), CoverageTag::Approximate);
    assert_eq!(money(&o), dec!(78000)); // only lump-sum, no fabricated subsidy
}

// ───────────────────────────── tax ─────────────────────────────

#[test]
fn re_28_tax_below_cap_exempt() {
    // 经济补偿 100000 < 社平 3 倍年额 (10923*36=393228) → 免税，税后 = 税前。
    let region = gd();
    let o = tax::switch(dec!(100000), &region).unwrap();
    assert_eq!(money(&o), dec!(100000));
}

#[test]
fn re_29_tax_above_cap_taxed() {
    // 经济补偿 500000 > 393228，应税 = 106772，税 = 106772*0.1-2520 = 8157.2，税后 = 491842.8.
    let region = gd();
    let o = tax::switch(dec!(500000), &region).unwrap();
    let cap = dec!(10923) * dec!(36);
    let taxable = dec!(500000) - cap;
    let tax_amt = (taxable * dec!(0.1) - dec!(2520)).round_dp(2);
    assert_eq!(money(&o), (dec!(500000) - tax_amt).round_dp(2));
}

#[test]
fn re_30_tax_after_tax_helper_monotonic() {
    let avg = dec!(10923);
    assert_eq!(tax::after_tax(dec!(50000), avg), dec!(50000));
    assert!(tax::after_tax(dec!(600000), avg) < dec!(600000));
}

// ───────────────────────────── litigation cost ─────────────────────────────

#[test]
fn re_31_litigation_cost_net_below_recovery() {
    let region = gd();
    let facts = FactBundle {
        overdue_wage_amount: dec!(50000),
        ..Default::default()
    };
    let o = litigation_cost::compare(&facts, &region).unwrap();
    let net = money(&o);
    assert!(
        net < dec!(50000) && net > dec!(0),
        "net within bounds: {net}"
    );
}

#[test]
fn re_31b_litigation_cost_missing_recovery_abstains() {
    let region = gd();
    assert!(!litigation_cost::compare(&FactBundle::default(), &region)
        .unwrap()
        .is_ok());
}

// ───────────────────────────── X threshold (RE-03, D11) ─────────────────────────────

#[test]
fn re03_32_x_threshold_resolves_out_of_scope_with_disclaimer() {
    let o = x_threshold::resolve_x_threshold(&FactBundle::default());
    match o {
        RuleOutcome::OutOfScope {
            coverage_tag,
            next_actions,
            ..
        } => {
            assert_eq!(coverage_tag, CoverageTag::Unknown);
            let has_disclaimer = next_actions.iter().any(|a| {
                matches!(a, RuleNextAction::ShowUiHint { text } if text.contains("无法律强制约束力"))
            });
            assert!(
                has_disclaimer,
                "D11: UI hint must state no legal binding force"
            );
        }
        _ => panic!("X threshold must be OutOfScope"),
    }
}

#[test]
#[should_panic(expected = "blocked by L0-04")]
fn re03_33_x_threshold_compute_body_is_blocked() {
    // The determinate formula is intentionally unimplemented (L0-04). The public path never calls
    // it, but the body must remain a block marker.
    let _ = x_threshold::compute_x_threshold(dec!(8000), dec!(10923));
}

// ───────────────────────────── engine façade ─────────────────────────────

#[test]
fn re_34_try_resolve_severance_via_engine() {
    let engine = RuleEngine::new().unwrap();
    let req = RuleRequest {
        intent: RuleIntent::Severance,
        facts: FactBundle {
            start_date: Some(d(2016, 3, 1)),
            end_date: Some(d(2024, 3, 1)),
            wage_records: flat_wage_records(d(2016, 3, 1), dec!(8000)),
            ..Default::default()
        },
        deadline_facts: None,
        province: "44".to_string(),
        city: "440300".to_string(),
        severance_pre_tax: None,
    };
    let o = engine.try_resolve(&req).expect("some outcome");
    assert_eq!(money(&o), dec!(64000));
}

#[test]
fn re_35_try_resolve_x_threshold_never_panics() {
    let engine = RuleEngine::new().unwrap();
    let req = RuleRequest {
        intent: RuleIntent::XThreshold,
        facts: FactBundle::default(),
        deadline_facts: None,
        province: "44".to_string(),
        city: String::new(),
        severance_pre_tax: None,
    };
    let o = engine.try_resolve(&req).expect("some outcome");
    assert!(!o.is_ok());
}

#[test]
fn re_36_try_resolve_unknown_province_returns_none() {
    let engine = RuleEngine::new().unwrap();
    let req = RuleRequest {
        intent: RuleIntent::Severance,
        facts: FactBundle::default(),
        deadline_facts: None,
        province: "99".to_string(),
        city: String::new(),
        severance_pre_tax: None,
    };
    assert!(engine.try_resolve(&req).is_none());
}
