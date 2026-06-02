//! `/deadline` route (backend/01 §1.2 / §1.11, module M5).
//!
//! Deadline engine (`crates/rule-engine`; spec ai/06): arbitration limitation interruption /
//! suspension / start anchored on `case_occurred_at`, 31-province params, 10% buffer + manual
//! second confirmation (INV-08). The handler maps the request onto [`rule_engine::DeadlineFacts`]
//! per kind and runs the real engine for each requested kind.

use axum::{extract::State, response::Response, routing::post, Json, Router};
use chrono::Utc;
use data_model::PlainDate;
use rule_engine::{DeadlineFacts, DeadlineKind, RuleIntent, RuleOutcome, RuleRequest};
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// `POST /deadline/run` request (backend/01 §1.11).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineReq {
    pub case_id: String,
    /// One or more deadline events (the `case_occurred_at` anchor + the requested kinds).
    pub events: Vec<DeadlineEvent>,
    pub province: String,
    pub city: String,
}

/// A single deadline event: the anchor date + which limitation kind to compute.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineEvent {
    /// 起算锚 — 知道或应当知道权利被侵害之日 (`case_occurred_at`).
    pub case_occurred_at: PlainDate,
    /// Which limitation to compute. Defaults to the general 1-year arbitration limitation.
    #[serde(default)]
    pub kind: Option<String>,
    /// 劳动关系是否存续 — labour relationship still active (wage-arrears special path).
    #[serde(default)]
    pub labor_relationship_active: bool,
    /// 劳动关系终止日 — termination date (required for the wage-arrears countdown when ended).
    #[serde(default)]
    pub labor_relationship_ended_at: Option<PlainDate>,
    /// 评估时点 — as-of date; defaults to today.
    #[serde(default)]
    pub as_of: Option<PlainDate>,
}

/// `POST /deadline/run` response (backend/01 §1.11).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineResp {
    /// Each item includes the 10% buffer + the INV-08 manual-confirm flag (always true).
    pub items: Vec<DeadlineItem>,
    /// Work-injury three-stage workflow, when applicable (R1b; null here).
    pub workflow_3_stages: Option<serde_json::Value>,
}

/// One computed deadline item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineItem {
    pub kind: String,
    /// The rule-engine outcome (Ok carries a `DeadlineValue` with `manual_confirm_required`).
    pub outcome: RuleOutcome,
    /// Convenience mirror of the INV-08 manual-confirm flag (always true for an Ok deadline).
    pub manual_confirm_required: bool,
}

/// `POST /performance/evaluate` request (M16 履行监控). Stateless like `/deadline/run`: the payment
/// schedule is supplied in the body; the real engine derives per-installment state + the
/// breach-triggered §250 enforcement countdown. No fabricated values.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReq {
    pub case_id: String,
    pub province: String,
    pub city: String,
    pub instrument_kind: rule_engine::InstrumentKind,
    pub effective_date: PlainDate,
    pub installments: Vec<rule_engine::PerformanceInstallment>,
    #[serde(default)]
    pub as_of: Option<PlainDate>,
}

/// Register `POST /deadline/run` + `POST /performance/evaluate`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/deadline/run", post(deadline_run))
        .route("/performance/evaluate", post(performance_evaluate))
}

async fn performance_evaluate(
    State(s): State<AppState>,
    Json(req): Json<PerformanceReq>,
) -> Response {
    let trace = s.new_trace_id();
    let Some(engine) = s.rule_engine() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    // Missing schedule → OutOfScope (C-C-6: never fabricate a 0); surfaced as BadRequest.
    if req.installments.is_empty() {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    }
    // Same Level-4 KB staleness gate as /deadline/run (the §250 countdown must not run on an
    // expired KB). Abort with E_KB_OUTDATED + record KbExpiredBlock (data/03 §3.5).
    let Some(manifest) = s.kb_manifest() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if kb::ensure_calculable(manifest.generated_at, Utc::now()).is_err() {
        if let Ok(case_id) = uuid::Uuid::parse_str(&req.case_id) {
            let _ = crate::audit_helper::append_state_change(
                &s,
                audit::AuditReason::KbExpiredBlock,
                case_id,
                serde_json::json!({
                    "case_id": req.case_id,
                    "op": "performance_evaluate",
                    "kb_age_days": s.kb_age_days(),
                }),
                &trace,
            )
            .await;
        }
        return responses::error(data_model::ErrorCode::KbOutdated, trace);
    }
    let Some(region) = engine.regions().get(&req.province) else {
        return responses::error(data_model::ErrorCode::RuleNoCoverage, trace);
    };
    let facts = rule_engine::PerformanceFacts {
        instrument_kind: req.instrument_kind,
        effective_date: req.effective_date,
        installments: req.installments.clone(),
        as_of: req.as_of.unwrap_or_else(|| Utc::now().date_naive()),
    };
    match rule_engine::evaluate_performance(&facts, region) {
        Ok(status) => responses::ok_200(status, trace),
        Err(_) => responses::error(data_model::ErrorCode::Internal, trace),
    }
}

fn parse_kind(raw: Option<&str>) -> DeadlineKind {
    match raw.unwrap_or("arbitration_general") {
        "arbitration_general" => DeadlineKind::ArbitrationGeneral,
        "arbitration_wage" => DeadlineKind::ArbitrationWage,
        "inspection" => DeadlineKind::Inspection,
        "appeal_first_instance" => DeadlineKind::AppealFirstInstance,
        "appeal_second_instance" => DeadlineKind::AppealSecondInstance,
        "enforcement" => DeadlineKind::Enforcement,
        "injury_recognition" => DeadlineKind::InjuryRecognition,
        "injury_assessment" => DeadlineKind::InjuryAssessment,
        "injury_benefit_payout" => DeadlineKind::InjuryBenefitPayout,
        "occupational_disease" => DeadlineKind::OccupationalDisease,
        _ => DeadlineKind::ArbitrationGeneral,
    }
}

async fn deadline_run(State(s): State<AppState>, Json(req): Json<DeadlineReq>) -> Response {
    let trace = s.new_trace_id();
    let Some(engine) = s.rule_engine() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if req.events.is_empty() {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    }

    // INV-04 / KB-02 staleness gate: the M5 deadline engine must not run on a Level-4 expired KB
    // (age >= 30 days). Abort with E_KB_OUTDATED (422) + record KbExpiredBlock (data/03 §3.5).
    let Some(manifest) = s.kb_manifest() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if kb::ensure_calculable(manifest.generated_at, Utc::now()).is_err() {
        if let Ok(case_id) = uuid::Uuid::parse_str(&req.case_id) {
            let _ = crate::audit_helper::append_state_change(
                &s,
                audit::AuditReason::KbExpiredBlock,
                case_id,
                serde_json::json!({
                    "case_id": req.case_id,
                    "op": "deadline_run",
                    "kb_age_days": s.kb_age_days(),
                }),
                &trace,
            )
            .await;
        }
        return responses::error(data_model::ErrorCode::KbOutdated, trace);
    }

    let mut items = Vec::new();
    for event in &req.events {
        let kind = parse_kind(event.kind.as_deref());
        let facts = DeadlineFacts {
            case_occurred_at: event.case_occurred_at,
            labor_relationship_active: event.labor_relationship_active,
            labor_relationship_ended_at: event.labor_relationship_ended_at,
            interrupt_events: Vec::new(),
            suspend_intervals: Vec::new(),
            as_of: event.as_of.unwrap_or_else(|| Utc::now().date_naive()),
            recognition_conclusion_at: None,
            assessment_conclusion_at: None,
        };
        let rule_req = RuleRequest {
            intent: RuleIntent::Deadline(kind),
            facts: Default::default(),
            deadline_facts: Some(facts),
            province: req.province.clone(),
            city: req.city.clone(),
            severance_pre_tax: None,
        };
        let Some(outcome) = engine.try_resolve(&rule_req) else {
            return responses::error(data_model::ErrorCode::RuleNoCoverage, trace);
        };
        let manual_confirm_required = matches!(&outcome, RuleOutcome::Ok { .. });
        items.push(DeadlineItem {
            kind: serde_json::to_value(kind)
                .ok()
                .and_then(|v| v.as_str().map(|x| x.to_string()))
                .unwrap_or_default(),
            outcome,
            manual_confirm_required,
        });
    }

    let resp = DeadlineResp {
        items,
        workflow_3_stages: None,
    };
    responses::ok_200(resp, trace)
}
