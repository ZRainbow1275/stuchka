//! `/compute` route (backend/01 §1.2 / §1.11, module M9).
//!
//! Rule-engine calculation (`crates/rule-engine`, zero AI / zero HTTP, INV-01). Money is
//! [`rust_decimal::Decimal`] (never f64). chrono dates per the workspace ruling. The handler maps
//! each requested scenario onto a [`rule_engine::RuleRequest`] and runs the real engine.

use axum::{extract::State, response::Response, routing::post, Json, Router};
use data_model::PlainDate;
use rule_engine::{ComputedValue, FactBundle, RuleIntent, RuleOutcome, RuleRequest, WageRecord};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// Money as a fixed-point decimal (never f64; persisted as TEXT).
pub type Money = rust_decimal::Decimal;

/// `POST /compute/run` request (backend/01 §1.11).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputeReq {
    pub case_id: String,
    /// 经济补偿 / 赔偿 / 二倍工资 / 加班费 / 50% 加付 / 工伤.
    pub scenarios: Vec<String>,
    pub province: String,
    pub city: String,
    pub wage_data: WageData,
    pub period: DateRange,
}

/// Wage inputs for the M9 formulas (the api projection of `rule_engine::FactBundle`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WageData {
    /// Monthly wage shortcut (used when no 12-month detail is supplied).
    #[serde(default)]
    pub monthly_wage: Option<Decimal>,
    /// Preceding-12-months wage detail (`{month, gross}` rows).
    #[serde(default)]
    pub wage_records: Vec<WageRow>,
    /// Weekday extended overtime hours.
    #[serde(default)]
    pub weekday_ot_hours: Option<Decimal>,
    /// Rest-day overtime hours.
    #[serde(default)]
    pub weekend_ot_hours: Option<Decimal>,
    /// Statutory-holiday overtime hours.
    #[serde(default)]
    pub holiday_ot_hours: Option<Decimal>,
    /// Contract-signed date (double-wage cut-off).
    #[serde(default)]
    pub contract_signed_at: Option<PlainDate>,
}

/// One `{month, gross}` wage row.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WageRow {
    pub month: PlainDate,
    pub gross: Decimal,
}

/// Inclusive date range (`from` = hire date, `to` = termination date).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateRange {
    pub from: PlainDate,
    pub to: PlainDate,
}

/// `POST /compute/run` response (backend/01 §1.11).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputeResp {
    pub results: Vec<ComputeResult>,
    pub pre_tax: Money,
    pub post_tax: Money,
    pub winning_cost_compare: serde_json::Value,
    pub rule_engine_version: String,
}

/// One scenario result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputeResult {
    pub scenario: String,
    /// The rule-engine outcome (Ok / OutOfScope), serialized verbatim.
    pub outcome: RuleOutcome,
    /// The monetary amount when the outcome carries one (else null).
    pub amount: Option<Money>,
}

/// Register `POST /compute/run`.
pub fn routes() -> Router<AppState> {
    Router::new().route("/compute/run", post(compute_run))
}

/// Map a scenario string onto a [`RuleIntent`]. Returns `None` for an unknown scenario.
fn scenario_to_intent(scenario: &str) -> Option<RuleIntent> {
    match scenario {
        "severance" | "经济补偿" | "economic_compensation" => Some(RuleIntent::Severance),
        "compensation" | "赔偿" | "economic_damage" => Some(RuleIntent::Compensation),
        "double_pay" | "二倍工资" => Some(RuleIntent::DoublePay),
        "overtime" | "加班费" => Some(RuleIntent::Overtime),
        "delay_50" | "50%加付" | "malicious_arrears_surcharge" => Some(RuleIntent::Delay50),
        "injury" | "工伤" => Some(RuleIntent::Injury),
        "litigation_cost" => Some(RuleIntent::LitigationCost),
        _ => None,
    }
}

fn build_fact_bundle(req: &ComputeReq) -> FactBundle {
    let wage_records = req
        .wage_data
        .wage_records
        .iter()
        .map(|r| WageRecord::new(r.month, r.gross))
        .collect();
    FactBundle {
        start_date: Some(req.period.from),
        end_date: Some(req.period.to),
        wage_records,
        monthly_wage: req.wage_data.monthly_wage,
        contract_signed_at: req.wage_data.contract_signed_at,
        weekday_ot_hours: req.wage_data.weekday_ot_hours.unwrap_or_default(),
        weekend_ot_hours: req.wage_data.weekend_ot_hours.unwrap_or_default(),
        holiday_ot_hours: req.wage_data.holiday_ot_hours.unwrap_or_default(),
        has_inspection_order_overdue: false,
        overdue_wage_amount: Decimal::ZERO,
        injury: None,
    }
}

async fn compute_run(State(s): State<AppState>, Json(req): Json<ComputeReq>) -> Response {
    let trace = s.new_trace_id();
    let Some(engine) = s.rule_engine() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };

    // INV-04 / KB-02 staleness gate: M9 compensation MUST NOT run on a Level-4 expired KB
    // (age >= 30 days). Check the live KB manifest before computing; abort with E_KB_OUTDATED
    // (422) and record the KbExpiredBlock audit entry (data/03 §3.5, kb freshness).
    let Some(manifest) = s.kb_manifest() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if kb::ensure_calculable(manifest.generated_at, chrono::Utc::now()).is_err() {
        if let Ok(case_id) = uuid::Uuid::parse_str(&req.case_id) {
            let _ = crate::audit_helper::append_state_change(
                &s,
                audit::AuditReason::KbExpiredBlock,
                case_id,
                serde_json::json!({
                    "case_id": req.case_id,
                    "op": "compute_run",
                    "kb_age_days": s.kb_age_days(),
                }),
                &trace,
            )
            .await;
        }
        return responses::error(data_model::ErrorCode::KbOutdated, trace);
    }

    let facts = build_fact_bundle(&req);
    let mut results = Vec::new();
    let mut pre_tax = Decimal::ZERO;
    let mut any_covered = false;

    for scenario in &req.scenarios {
        let Some(intent) = scenario_to_intent(scenario) else {
            return responses::error(data_model::ErrorCode::RuleNoCoverage, trace);
        };
        let rule_req = RuleRequest {
            intent,
            facts: facts.clone(),
            deadline_facts: None,
            province: req.province.clone(),
            city: req.city.clone(),
            severance_pre_tax: None,
        };
        let Some(outcome) = engine.try_resolve(&rule_req) else {
            // Unknown province → no coverage.
            return responses::error(data_model::ErrorCode::RuleNoCoverage, trace);
        };
        let amount = match &outcome {
            RuleOutcome::Ok {
                value: ComputedValue::Money(m),
                ..
            } => {
                any_covered = true;
                pre_tax += *m;
                Some(*m)
            }
            _ => None,
        };
        results.push(ComputeResult {
            scenario: scenario.clone(),
            outcome,
            amount,
        });
    }

    if !any_covered {
        // Every scenario abstained — surface the rule-no-coverage contract code.
        return responses::error(data_model::ErrorCode::RuleNoCoverage, trace);
    }

    // Post-tax: apply the comprehensive-income switch to the aggregate pre-tax severance.
    let post_tax = match engine.regions().get(&req.province) {
        Some(region) => rule_engine::compute::tax::after_tax(pre_tax, region.avg_monthly_wage()),
        None => pre_tax,
    };

    let resp = ComputeResp {
        results,
        pre_tax,
        post_tax,
        winning_cost_compare: serde_json::Value::Null,
        rule_engine_version: rule_engine::CRATE_NAME.to_string(),
    };
    responses::ok_200(resp, trace)
}
