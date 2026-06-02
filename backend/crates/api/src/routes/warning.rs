//! `/case/:id/warnings` route (M12 风险预警, prd/04 §4.8). A read-only proactive-warning FEED
//! derived from the case's REAL persisted `case_occurred_at` + the M5 deadline engine — it
//! fabricates nothing (a kind that does not resolve, or is > 10 days out, yields no item).
//!
//! Three warning classes (§4.8.2):
//! - **时效到期预警** (deadline expiry, 10/3/1-day tiers): fully real here, via
//!   [`rule_engine::deadline::warning::warning_for`] over the engine's buffered remaining days.
//! - **法规变更** (regulation change, INV-04 hint-only on frozen cases): the persisted
//!   `ImpactNotice` store is a follow-up; this feed surfaces zero regulation_change items until it
//!   lands (honest — it never fabricates a change notice).
//! - **证据灭失风险** (evidence-loss): R1 has NO data model for third-party-data retention /
//!   query-window-close dates, so this is an explicit HONEST SEAM (`evidence_loss_seam`) emitting
//!   ZERO evidence_loss items rather than inventing retention dates (mirrors Wave-2 cert/GPG seams).

use axum::{
    extract::{Path, State},
    response::Response,
    routing::get,
    Router,
};
use chrono::Utc;
use rule_engine::{ComputedValue, DeadlineFacts, DeadlineKind, RuleIntent, RuleOutcome, RuleRequest};
use rule_engine::deadline::warning::warning_for;
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// `GET /case/:id/warnings` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningFeedResp {
    pub case_id: String,
    pub items: Vec<WarningItem>,
    /// Honest seam: evidence-loss warnings need data the R1 model does not carry (see module doc).
    pub evidence_loss_seam: EvidenceLossSeam,
}

/// One proactive-warning item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningItem {
    /// `deadline` | `regulation_change`.
    pub class: String,
    pub kind: String,
    /// `t10_days` | `t3_days` | `t1_day`.
    pub tier: String,
    pub tier_label: String,
    pub buffered_remaining_days: i64,
    pub raw_remaining_days: i64,
    /// Always true for a deadline item (INV-08).
    pub manual_confirm_required: bool,
    pub law_refs: Vec<String>,
}

/// The evidence-loss honest seam (declares the gap rather than fabricating retention dates).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLossSeam {
    pub available: bool,
    pub reason: String,
    pub required_fields: Vec<String>,
}

impl Default for EvidenceLossSeam {
    fn default() -> Self {
        Self {
            available: false,
            reason: "证据灭失预警需第三方数据保存期 / 查询窗口关闭日字段，R1 数据模型未建模".to_string(),
            required_fields: vec![
                "third_party_retention_until".to_string(),
                "query_window_closes_at".to_string(),
            ],
        }
    }
}

/// Register `GET /case/:id/warnings`.
pub fn routes() -> Router<AppState> {
    Router::new().route("/case/:id/warnings", get(case_warnings))
}

/// The limitation kinds anchored purely on `case_occurred_at` that apply to every labour dispute
/// (the universal arbitration clock + the labour-inspection clock). Kinds needing extra facts not
/// stored on `Case` (wage-active flag, injury conclusion dates) are intentionally NOT forced here —
/// they would `OutOfScope` and be skipped (never fabricated).
const FEED_KINDS: [DeadlineKind; 2] = [DeadlineKind::ArbitrationGeneral, DeadlineKind::Inspection];

async fn case_warnings(Path(id): Path<String>, State(s): State<AppState>) -> Response {
    let trace = s.new_trace_id();
    let Some(engine) = s.rule_engine() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(case_id) = uuid::Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };
    let case = match store.case.get(&case_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(_) => return responses::error(data_model::ErrorCode::Internal, trace),
    };

    // Same Level-4 KB staleness gate as /deadline/run: a warning feed must not run on an expired KB.
    let Some(manifest) = s.kb_manifest() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if kb::ensure_calculable(manifest.generated_at, Utc::now()).is_err() {
        let _ = crate::audit_helper::append_state_change(
            &s,
            audit::AuditReason::KbExpiredBlock,
            case_id,
            serde_json::json!({"case_id": id, "op": "case_warnings", "kb_age_days": s.kb_age_days()}),
            &trace,
        )
        .await;
        return responses::error(data_model::ErrorCode::KbOutdated, trace);
    }

    let as_of = Utc::now().date_naive();
    let mut items = Vec::new();
    for kind in FEED_KINDS {
        let facts = DeadlineFacts {
            case_occurred_at: case.case_occurred_at,
            labor_relationship_active: false,
            labor_relationship_ended_at: None,
            interrupt_events: Vec::new(),
            suspend_intervals: Vec::new(),
            as_of,
            recognition_conclusion_at: None,
            assessment_conclusion_at: None,
        };
        let rule_req = RuleRequest {
            intent: RuleIntent::Deadline(kind),
            facts: Default::default(),
            deadline_facts: Some(facts),
            province: case.province.clone(),
            city: case.city.clone(),
            severance_pre_tax: None,
        };
        // OutOfScope / no coverage → skip (NEVER a fabricated warning).
        let Some(RuleOutcome::Ok { value: ComputedValue::Deadline(v), .. }) =
            engine.try_resolve(&rule_req)
        else {
            continue;
        };
        if let Some((tier, buffered)) = warning_for(&v) {
            items.push(WarningItem {
                class: "deadline".to_string(),
                kind: serde_json::to_value(v.kind)
                    .ok()
                    .and_then(|x| x.as_str().map(str::to_string))
                    .unwrap_or_default(),
                tier: tier.as_str().to_string(),
                tier_label: tier.label_zh().to_string(),
                buffered_remaining_days: buffered,
                raw_remaining_days: v.raw_remaining_days,
                manual_confirm_required: true,
                law_refs: v.law_refs.iter().map(|r| r.urn.clone()).collect(),
            });
        }
    }

    let resp = WarningFeedResp {
        case_id: id,
        items,
        evidence_loss_seam: EvidenceLossSeam::default(),
    };
    responses::ok_200(resp, trace)
}
