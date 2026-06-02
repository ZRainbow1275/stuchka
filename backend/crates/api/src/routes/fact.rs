//! `/fact` routes (backend/01 §1.2 / §1.4, module M1).
//!
//! `POST /case/:id/fact` and `GET /case/:id/fact` (the handlers live here, registered under the
//! case tree in `routes/case.rs`); this module also owns `PATCH /fact/:id` plus the DTOs. chrono
//! time types per the workspace ruling. Handlers call the real db `FactRepo` + audit.

use axum::{
    extract::{Path, State},
    response::Response,
    routing::patch,
    Json, Router,
};
use chrono::Utc;
use data_model::{
    new_id, validate_fact_transition, CaseFact, CoverageTag, FactCategory, FactSource, FactStatus,
    PlainDate, Timestamp,
};
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// The M15 authorization-chain link (backend/01 §1.4.1). Canonical type lives in `data-model`
/// (cross-crate reconciliation B-7); re-exported here so `crate::routes::fact::AuthLink` (used by
/// the sync route module + this handler) keeps resolving.
pub use data_model::AuthLink;

/// `POST /case/:id/fact` request (backend/01 §1.4.1).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFactReq {
    pub category: FactCategory,
    pub statement: String,
    pub occurred_at: Option<PlainDate>,
    pub source: FactSource,
    pub group_id: Option<String>,
    pub contributor_id: Option<String>,
    #[serde(default)]
    pub authorization_chain: Vec<AuthLink>,
}

/// Fact DTO (mirror of [`data_model::CaseFact`]) returned by the case aggregate + fact handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactDto {
    pub id: String,
    pub case_id: String,
    pub content: String,
    pub category: FactCategory,
    pub status: FactStatus,
    pub source: FactSource,
    pub confidence: Option<f32>,
    pub coverage_tag: CoverageTag,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl FactDto {
    /// Project the persisted [`CaseFact`] into the DTO.
    pub fn from_fact(f: &CaseFact) -> Self {
        Self {
            id: f.id.to_string(),
            case_id: f.case_id.to_string(),
            content: f.content.clone(),
            category: f.category,
            status: f.status,
            source: f.source,
            confidence: f.confidence,
            coverage_tag: f.coverage_tag,
            created_at: f.created_at,
            updated_at: f.updated_at,
        }
    }
}

/// `PATCH /fact/:id` request (backend/01 §1.4.2).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactTransitionReq {
    /// `confirm` | `dispute` | `deprecate`.
    pub action: String,
    pub reason: String,
}

/// Register `PATCH /fact/:id`.
pub fn routes() -> Router<AppState> {
    Router::new().route("/fact/:id", patch(transition_fact))
}

/// `POST /case/:id/fact` — create a new fact under a case (registered under the case tree).
pub async fn create_fact(
    State(s): State<AppState>,
    Path(case_id): Path<String>,
    Json(req): Json<CreateFactReq>,
) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(case_uuid) = uuid::Uuid::parse_str(&case_id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    // Confirm the case exists (FK RESTRICT would also reject, but a clean 404 is nicer).
    match store.case.get(&case_uuid).await {
        Ok(Some(_)) => {}
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    }

    let now = Utc::now();
    let auth_chain = if req.authorization_chain.is_empty() {
        None
    } else {
        Some(serde_json::to_value(&req.authorization_chain).unwrap_or(serde_json::Value::Null))
    };
    let fact = CaseFact {
        id: new_id(),
        case_id: case_uuid,
        content: req.statement,
        category: req.category,
        status: FactStatus::Pending,
        source: req.source,
        confidence: None,
        coverage_tag: CoverageTag::Unknown,
        evidence_refs: Vec::new(),
        group_id: req
            .group_id
            .as_deref()
            .and_then(|g| uuid::Uuid::parse_str(g).ok()),
        contributor_id: req
            .contributor_id
            .as_deref()
            .and_then(|c| uuid::Uuid::parse_str(c).ok()),
        authorization_chain: auth_chain,
        created_at: now,
        updated_at: now,
    };

    if let Err(e) = store.fact.insert(&fact).await {
        return responses::from_db_error(&e, trace);
    }

    if let Err(resp) = crate::audit_helper::append(
        &s,
        audit::Subject::System {
            component: "fact".to_string(),
        },
        audit::AuditReason::FactStateTransition,
        case_uuid,
        serde_json::json!({
            "case_id": case_uuid.to_string(),
            "fact_id": fact.id.to_string(),
            "action": "create",
            "status": "pending",
        }),
        &trace,
    )
    .await
    {
        return resp;
    }

    responses::created(FactDto::from_fact(&fact), trace)
}

/// `GET /case/:id/fact` — list facts for a case (registered under the case tree).
pub async fn list_facts(State(s): State<AppState>, Path(case_id): Path<String>) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(case_uuid) = uuid::Uuid::parse_str(&case_id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };
    match store.fact.list_by_case(&case_uuid).await {
        Ok(fs) => {
            let dtos: Vec<FactDto> = fs.iter().map(FactDto::from_fact).collect();
            responses::ok_200(dtos, trace)
        }
        Err(e) => responses::from_db_error(&e, trace),
    }
}

async fn transition_fact(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<FactTransitionReq>,
) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(fact_id) = uuid::Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    let mut fact = match store.fact.get(&fact_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    };

    let target = match req.action.as_str() {
        "confirm" => FactStatus::Confirmed,
        "dispute" => FactStatus::Disputed,
        "deprecate" => FactStatus::Deprecated,
        _ => return responses::error(data_model::ErrorCode::BadRequest, trace),
    };
    if validate_fact_transition(fact.status, target).is_err() {
        return responses::error(data_model::ErrorCode::InvalidTransition, trace);
    }

    let from = fact.status;
    let case_uuid = fact.case_id;
    fact.status = target;
    fact.updated_at = Utc::now();
    if let Err(e) = store.fact.update(&fact).await {
        return responses::from_db_error(&e, trace);
    }

    let why = match target {
        FactStatus::Confirmed => audit::AuditReason::UserConfirmFact,
        FactStatus::Disputed => audit::AuditReason::UserDisputeFact,
        _ => audit::AuditReason::FactStateTransition,
    };
    if let Err(resp) = crate::audit_helper::append(
        &s,
        audit::Subject::System {
            component: "fact".to_string(),
        },
        why,
        case_uuid,
        serde_json::json!({
            "case_id": case_uuid.to_string(),
            "fact_id": fact.id.to_string(),
            "from": serde_json::to_value(from).unwrap_or(serde_json::Value::Null),
            "to": serde_json::to_value(target).unwrap_or(serde_json::Value::Null),
            "reason": req.reason,
        }),
        &trace,
    )
    .await
    {
        return resp;
    }

    responses::ok_200(FactDto::from_fact(&fact), trace)
}
