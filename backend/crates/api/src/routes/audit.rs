//! `/audit` routes (backend/01 §1.2 / §1.8, INV-06).
//!
//! Data source is the independent `audit.sqlite` (`crates/audit`, D3) — not the main store.
//! `GET /audit` (R1) is a decrypted projection of the chain (optionally filtered by `case_id`);
//! `POST /audit/anchor` (R1b) triggers a real OTS upload and stays `501 E_NOT_IMPLEMENTED` until
//! the network anchoring path lands (W8: R1a ships the OTS trait + mock + CLI seam, the real
//! calendar upload is R1b).

use audit::AuditReason;
use axum::{
    extract::{Query, State},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use data_model::{AuditCategory, Timestamp};
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// Audit entry projection DTO (backend/01 §1.8). Field source is `audit.sqlite` (data/04).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntryDto {
    pub seq: i64,
    pub case_id: Option<String>,
    pub who: String,
    pub when: Timestamp,
    pub why: String,
    pub what: serde_json::Value,
    pub category: AuditCategory,
    pub prev_hash: String,
    pub record_hash: String,
    pub ots_proof: Option<String>,
}

/// `GET /audit` query (backend/01 §1.8): `?case_id=&from=&to=&category=&cursor=&limit=`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    pub case_id: Option<String>,
    pub from: Option<Timestamp>,
    pub to: Option<Timestamp>,
    pub category: Option<AuditCategory>,
    /// Cursor based on `seq`.
    pub cursor: Option<i64>,
    pub limit: Option<u32>,
}

/// `POST /audit/ack` request — a high-risk (INV-10) disclaimer acknowledgement (compliance/05 §2.3
/// audit four-tuple). The frontend `HighRiskGate` posts this on the SECOND (final) confirm so the
/// acknowledgement is persisted to the independent `audit.sqlite` hash chain (INV-06), not dropped.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighRiskAckReq {
    /// The case the acknowledgement belongs to (case-scoped, schema requires a UUID).
    pub case_id: String,
    /// The compliance/05 §1 stable scene id (`S-01`..`S-06`, or `GROUP-CREATE`).
    pub scene_id: String,
    /// Measured INV-10 two-stage cooldown timings (compliance/05 §2.3 / §9.1).
    pub timings: ConfirmTimingsDto,
}

/// The measured INV-10 confirm-flow timings (mirror of the Flutter `ConfirmTimings`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmTimingsDto {
    /// Whether a mandatory second press was required (always true for the INV-10 gate, §9.1).
    pub requires_second_press: bool,
    /// First (read) countdown actually-elapsed ms; `cooldown_actual_ms` must be >= 8000 (§2.3/§9.1).
    pub first_countdown_ms: i64,
    /// Gap between the first press and the final commit (>= 3000ms, §9.1).
    pub second_press_gap_ms: i64,
    /// Total open->commit flow (>= 11000ms, §5.1/§9.1).
    pub total_flow_ms: i64,
}

/// `POST /audit/ack` response — the appended record's `seq` (so the caller can confirm persistence).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighRiskAckResp {
    pub seq: i64,
}

/// Map a compliance/05 §1 stable scene id onto its INV-10 audit reason (data/04 §4.4 `Inv10*`).
fn scene_to_reason(scene_id: &str) -> Option<AuditReason> {
    match scene_id {
        "S-01" => Some(AuditReason::Inv10VoluntaryResign),
        "S-02" => Some(AuditReason::Inv10SettlementBelow80),
        "S-03" => Some(AuditReason::Inv10ClaimWithdraw),
        "S-04" => Some(AuditReason::Inv10GroupRepresentAuth),
        "S-05" => Some(AuditReason::Inv10CriminalReportExport),
        // S-06 covers 医疗期 / 三期 / 未成年 / 工伤 / 性骚扰; the audit reason set distinguishes the
        // first three — the protected-scenario gate maps to the medical-leave reason as the
        // representative §4.4 trigger (the concrete category travels in `what.scene_id`).
        "S-06" => Some(AuditReason::Inv10MedicalLeave),
        _ => None,
    }
}

/// Register `GET /audit`, `POST /audit/ack`, and `POST /audit/anchor`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/audit", get(query_audit))
        .route("/audit/ack", post(ack_high_risk))
        .route("/audit/anchor", post(anchor_audit))
}

/// `POST /audit/ack` — append an INV-10 high-risk disclaimer acknowledgement to the independent
/// `audit.sqlite` hash chain (compliance/05 §2.3 + data/04 §4.4 `Inv10*`, INV-06 append-only).
///
/// The four-tuple is: who = system (R1a has no per-user identity), when = `Utc::now()` (set inside
/// `AuditLog::append`), why = the §4.4 `Inv10*` reason, what = `{ case_id, scene_id, timings,
/// acknowledged_at }`. A broken startup chain refuses the write (`E_AUDIT_CHAIN_BROKEN`).
async fn ack_high_risk(State(s): State<AppState>, Json(req): Json<HighRiskAckReq>) -> Response {
    let trace = s.new_trace_id();

    let Ok(case_id) = uuid::Uuid::parse_str(&req.case_id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };
    let Some(reason) = scene_to_reason(&req.scene_id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    // INV-10 §2.3 floor: a < 8000ms cooldown means the UI shortened the read window — reject it so a
    // tampered/short flow can never be persisted as a valid acknowledgement.
    if req.timings.first_countdown_ms < 8000 {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    }

    let what = serde_json::json!({
        "case_id": req.case_id,
        "scene_id": req.scene_id,
        "ui_reason": "high_risk_ack",
        "timings": {
            "requiresSecondPress": req.timings.requires_second_press,
            "cooldownActualMs": req.timings.first_countdown_ms,
            "secondPressGapMs": req.timings.second_press_gap_ms,
            "totalFlowMs": req.timings.total_flow_ms,
        },
        "acknowledged_at": chrono::Utc::now().to_rfc3339(),
    });

    match crate::audit_helper::append_state_change(&s, reason, case_id, what, &trace).await {
        Ok(seq) => responses::ok_200(HighRiskAckResp { seq }, trace),
        Err(resp) => resp,
    }
}

async fn query_audit(State(s): State<AppState>, Query(query): Query<AuditQuery>) -> Response {
    let trace = s.new_trace_id();
    let Some(audit) = s.audit() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if !s.audit_chain_ok() {
        return responses::error(data_model::ErrorCode::AuditChainBroken, trace);
    }

    // R1 supports case-scoped projection (the chain index `query_by_case`); an unfiltered global
    // stream is a later subtask. A missing/invalid case_id yields an empty projection.
    let Some(case_id_str) = query.case_id else {
        return responses::ok_200(Vec::<AuditEntryDto>::new(), trace);
    };
    let Ok(case_id) = uuid::Uuid::parse_str(&case_id_str) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    match audit.query_by_case(case_id).await {
        Ok(records) => {
            let dtos: Vec<AuditEntryDto> = records
                .into_iter()
                .map(|r| AuditEntryDto {
                    seq: r.seq,
                    case_id: r.case_id.map(|c| c.to_string()),
                    who: r.who.who_display(),
                    when: r.when,
                    why: r.why.as_str().to_string(),
                    what: r.what,
                    category: r.why.category(),
                    prev_hash: r.prev_hash,
                    record_hash: r.record_hash,
                    ots_proof: None,
                })
                .collect();
            responses::ok_200(dtos, trace)
        }
        Err(e) => {
            tracing::error!(error = %e, "audit query failed");
            responses::error(data_model::ErrorCode::Internal, trace)
        }
    }
}

// R1b: real OpenTimestamps calendar upload. Kept E_NOT_IMPLEMENTED until the network anchoring
// path ships (W8: R1a = trait + mock + CLI seam only).
async fn anchor_audit(State(s): State<AppState>) -> Response {
    responses::not_implemented(s.new_trace_id())
}
