//! `/audit` routes (backend/01 §1.2 / §1.8, INV-06).
//!
//! Data source is the independent `audit.sqlite` (`crates/audit`, D3) — not the main store.
//! `GET /audit` (R1) is a decrypted projection of the chain (optionally filtered by `case_id`);
//! `POST /audit/anchor` (R1b) triggers a real OTS upload and stays `501 E_NOT_IMPLEMENTED` until
//! the network anchoring path lands (W8: R1a ships the OTS trait + mock + CLI seam, the real
//! calendar upload is R1b).

use axum::{
    extract::{Query, State},
    response::Response,
    routing::{get, post},
    Router,
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

/// Register `GET /audit` and `POST /audit/anchor`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/audit", get(query_audit))
        .route("/audit/anchor", post(anchor_audit))
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
