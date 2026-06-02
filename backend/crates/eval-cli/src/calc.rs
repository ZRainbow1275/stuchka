//! calc-50 gate (ai/05 §5.3 + ai/02 §2.6): M9 计算正确率, 100% 错一道不发版.
//!
//! Drives the REAL `rule_engine::RuleEngine::try_resolve` (the Stage-D entry) for every case and
//! compares the produced money / out-of-scope outcome against the law-derived expectation in the
//! dataset. Expected values are the verbatim golden outputs of the law formulas (mirroring
//! `rule-engine/tests/calc_compute.rs`) plus cross-province non-capped permutations whose value is
//! `wage x service-years` — never fabricated.

use chrono::NaiveDate;
use rule_engine::compute::injury::InjuryInputs;
use rule_engine::facts::{FactBundle, WageRecord};
use rule_engine::{ComputedValue, RuleEngine, RuleIntent, RuleOutcome, RuleRequest};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::common::{coverage_tag_str, read_json};
use crate::report::{CaseResult, GateReport};

#[derive(Debug, Deserialize)]
struct InjurySpec {
    disability_grade: u8,
    avg_monthly_wage_pre_12: Decimal,
    region_avg_wage: Decimal,
    region_min_wage: Decimal,
    #[serde(default)]
    is_renouncing_labor: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Expect {
    Money {
        amount: Decimal,
        #[serde(default)]
        coverage_tag: Option<String>,
    },
    OutOfScope {
        #[serde(default)]
        coverage_tag: Option<String>,
        #[serde(default)]
        next_action_field: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct CalcCase {
    id: String,
    /// One of: severance|compensation|double_pay|overtime|delay_50|injury|tax|litigation_cost|x_threshold
    intent: String,
    province: String,
    #[serde(default)]
    city: String,
    #[serde(default)]
    start_date: Option<NaiveDate>,
    #[serde(default)]
    end_date: Option<NaiveDate>,
    /// Expands to 12 flat monthly wage records anchored at `wage_records_from` (or `start_date`).
    #[serde(default)]
    flat_monthly_wage: Option<Decimal>,
    /// Anchor for the flat wage records when it differs from `start_date` (e.g. missing-start
    /// abstention cases that still carry wage history).
    #[serde(default)]
    wage_records_from: Option<NaiveDate>,
    #[serde(default)]
    monthly_wage: Option<Decimal>,
    #[serde(default)]
    contract_signed_at: Option<NaiveDate>,
    #[serde(default)]
    weekday_ot_hours: Option<Decimal>,
    #[serde(default)]
    weekend_ot_hours: Option<Decimal>,
    #[serde(default)]
    holiday_ot_hours: Option<Decimal>,
    #[serde(default)]
    has_inspection_order_overdue: bool,
    #[serde(default)]
    overdue_wage_amount: Option<Decimal>,
    #[serde(default)]
    injury: Option<InjurySpec>,
    #[serde(default)]
    severance_pre_tax: Option<Decimal>,
    expect: Expect,
}

#[derive(Debug, Deserialize)]
struct CalcDataset {
    cases: Vec<CalcCase>,
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

fn intent_of(s: &str) -> Result<RuleIntent, String> {
    Ok(match s {
        "severance" => RuleIntent::Severance,
        "compensation" => RuleIntent::Compensation,
        "double_pay" => RuleIntent::DoublePay,
        "overtime" => RuleIntent::Overtime,
        "delay_50" => RuleIntent::Delay50,
        "injury" => RuleIntent::Injury,
        "tax" => RuleIntent::Tax,
        "litigation_cost" => RuleIntent::LitigationCost,
        "x_threshold" => RuleIntent::XThreshold,
        other => return Err(format!("unknown calc intent '{other}'")),
    })
}

fn build_request(case: &CalcCase) -> Result<RuleRequest, String> {
    let wage_anchor = case.wage_records_from.or(case.start_date);
    let wage_records = match (case.flat_monthly_wage, wage_anchor) {
        (Some(w), Some(anchor)) => flat_wage_records(anchor, w),
        (Some(_), None) => {
            return Err(format!(
                "case {}: flat_monthly_wage requires start_date or wage_records_from",
                case.id
            ))
        }
        (None, _) => Vec::new(),
    };
    let facts = FactBundle {
        start_date: case.start_date,
        end_date: case.end_date,
        wage_records,
        monthly_wage: case.monthly_wage,
        contract_signed_at: case.contract_signed_at,
        weekday_ot_hours: case.weekday_ot_hours.unwrap_or(Decimal::ZERO),
        weekend_ot_hours: case.weekend_ot_hours.unwrap_or(Decimal::ZERO),
        holiday_ot_hours: case.holiday_ot_hours.unwrap_or(Decimal::ZERO),
        has_inspection_order_overdue: case.has_inspection_order_overdue,
        overdue_wage_amount: case.overdue_wage_amount.unwrap_or(Decimal::ZERO),
        injury: case.injury.as_ref().map(|i| InjuryInputs {
            disability_grade: i.disability_grade,
            avg_monthly_wage_pre_12: i.avg_monthly_wage_pre_12,
            region_avg_wage: i.region_avg_wage,
            region_min_wage: i.region_min_wage,
            is_renouncing_labor: i.is_renouncing_labor,
        }),
    };
    Ok(RuleRequest {
        intent: intent_of(&case.intent)?,
        facts,
        deadline_facts: None,
        province: case.province.clone(),
        city: case.city.clone(),
        severance_pre_tax: case.severance_pre_tax,
    })
}

fn evaluate(case: &CalcCase, outcome: &RuleOutcome) -> CaseResult {
    match (&case.expect, outcome) {
        (
            Expect::Money {
                amount,
                coverage_tag,
            },
            RuleOutcome::Ok {
                value: ComputedValue::Money(got),
                coverage_tag: got_tag,
                ..
            },
        ) => {
            let tag_ok = coverage_tag
                .as_deref()
                .is_none_or(|t| coverage_tag_str(*got_tag) == t);
            let pass = got == amount && tag_ok;
            CaseResult::new(
                &case.id,
                pass,
                format!(
                    "money {amount}{}",
                    coverage_tag
                        .as_deref()
                        .map(|t| format!(" ({t})"))
                        .unwrap_or_default()
                ),
                format!("money {got} ({})", coverage_tag_str(*got_tag)),
            )
        }
        (
            Expect::OutOfScope {
                coverage_tag,
                next_action_field,
            },
            RuleOutcome::OutOfScope {
                coverage_tag: got_tag,
                next_actions,
                ..
            },
        ) => {
            let tag_ok = coverage_tag
                .as_deref()
                .is_none_or(|t| coverage_tag_str(*got_tag) == t);
            let field_ok = next_action_field.as_deref().is_none_or(|want| {
                next_actions.iter().any(|a| match a {
                    rule_engine::RuleNextAction::CollectFact { field } => field == want,
                    _ => false,
                })
            });
            let pass = tag_ok && field_ok;
            CaseResult::new(
                &case.id,
                pass,
                format!(
                    "out_of_scope{}{}",
                    coverage_tag
                        .as_deref()
                        .map(|t| format!(" tag={t}"))
                        .unwrap_or_default(),
                    next_action_field
                        .as_deref()
                        .map(|f| format!(" field={f}"))
                        .unwrap_or_default()
                ),
                format!("out_of_scope tag={}", coverage_tag_str(*got_tag)),
            )
        }
        (Expect::Money { amount, .. }, other) => CaseResult::new(
            &case.id,
            false,
            format!("money {amount}"),
            format!("{other:?}"),
        ),
        (Expect::OutOfScope { .. }, other) => {
            CaseResult::new(&case.id, false, "out_of_scope", format!("{other:?}"))
        }
    }
}

pub fn run(dataset: &std::path::Path) -> Result<GateReport, String> {
    let data: CalcDataset = read_json(dataset)?;
    let engine = RuleEngine::new().map_err(|e| format!("rule engine init: {e}"))?;
    let mut cases = Vec::with_capacity(data.cases.len());
    for case in &data.cases {
        let req = build_request(case)?;
        match engine.try_resolve(&req) {
            Some(outcome) => cases.push(evaluate(case, &outcome)),
            None => cases.push(
                CaseResult::new(&case.id, false, "outcome", "None")
                    .with_note(format!("unknown province '{}'", case.province)),
            ),
        }
    }
    Ok(GateReport::from_cases("calc", cases))
}
