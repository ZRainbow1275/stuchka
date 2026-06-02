//! `/case` routes (backend/01 §1.2 / §1.3, module M1).
//!
//! DTOs match the spec contract; time fields use the workspace chrono types
//! ([`data_model::PlainDate`] / [`data_model::Timestamp`]) per the chrono ruling (the spec's
//! `jiff` types are superseded). Handlers call the real db / kb / audit / rule-engine module
//! crates (no stubs); `/case/:id/fact` + `/case/:id/evidence` bodies live in the fact / evidence
//! modules' handler functions, registered under the case path tree here per §1.2.

use axum::{
    extract::{Path, State},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use data_model::{
    new_id, validate_case_transition, Case, CaseStatus, CoverageTag, CoverageTier, DisputeSubtype,
    IdentityType, PlainDate, Timestamp,
};
use serde::{Deserialize, Serialize};

use crate::routes::evidence::EvidenceDto;
use crate::routes::fact::FactDto;
use crate::{responses, state::AppState};

/// Pre-diagnosis placeholder for the `dispute_category` column (the schema requires a well-formed
/// `LD-NN-NN`; M1 diagnosis replaces this). Surfaced as `disputeCategoryId: null` in the DTO.
const UNDIAGNOSED_CATEGORY: &str = "LD-00-00";

/// `POST /case` request (backend/01 §1.3.1).
///
/// When `answerPath` is supplied (a completed 问诊树 path, prd §4.1.4), the deterministic M1
/// [`rule_engine::DiagnosisEngine`] runs at creation time and the REAL diagnosed `LD-NN-NN`
/// subcategory + coverage tier are stored (replacing the `LD-00-00` placeholder) and the case opens
/// as `diagnosed` (INV-01 pure rules). When it is omitted (or the path is incomplete / abstains),
/// the case is created `draft` with the placeholder, and the diagnosis is run later via
/// `POST /diagnose` — keeping the legacy create-then-diagnose flow backward-compatible.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCaseReq {
    pub identity_type: IdentityType,
    pub province: String,
    pub city: String,
    pub region_code: Option<String>,
    pub case_occurred_at: PlainDate,
    pub dispute_subtype: DisputeSubtype,
    pub first_description: String,
    pub kb_version_hash: String,
    /// Optional completed 问诊树 answer path; when present, M1 diagnosis runs at creation.
    #[serde(default)]
    pub answer_path: Vec<String>,
    /// Questions the user skipped during 问诊 (≥ 3 → abstention, prd §4.1.4).
    #[serde(default)]
    pub skipped_questions: u32,
}

/// Case DTO (backend/01 §1.3.1). `id` carries a UUID v7 36-char string (D9).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseDto {
    pub id: String,
    pub status: CaseStatus,
    pub identity_type: IdentityType,
    pub province: String,
    pub city: String,
    pub region_code: Option<String>,
    pub case_occurred_at: PlainDate,
    pub dispute_subtype: DisputeSubtype,
    pub dispute_category_id: Option<String>,
    pub coverage_tier: Option<CoverageTier>,
    pub coverage_tag: Option<CoverageTag>,
    pub kb_version_hash: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl CaseDto {
    /// Project the persisted [`Case`] aggregate into the DTO.
    pub fn from_case(c: &Case) -> Self {
        Self {
            id: c.id.to_string(),
            status: c.status,
            identity_type: c.identity_type,
            province: c.province.clone(),
            city: c.city.clone(),
            region_code: c.region_code.clone(),
            case_occurred_at: c.case_occurred_at,
            dispute_subtype: c.dispute_subtype,
            dispute_category_id: if c.dispute_category.is_empty()
                || c.dispute_category == UNDIAGNOSED_CATEGORY
            {
                None
            } else {
                Some(c.dispute_category.clone())
            },
            coverage_tier: Some(c.coverage_tier),
            coverage_tag: None,
            kb_version_hash: c.kb_version_hash.clone(),
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

/// Aggregate view returned by `GET /case/:id` (backend/01 §1.3.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseAggregateDto {
    #[serde(flatten)]
    pub case: CaseDto,
    pub facts: Vec<FactDto>,
    pub evidences: Vec<EvidenceDto>,
    pub documents: Vec<serde_json::Value>,
    pub claims: Vec<serde_json::Value>,
    pub deadlines: Vec<serde_json::Value>,
}

/// `PATCH /case/:id` request (backend/01 §1.3.3).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseTransitionReq {
    /// `confirm` | `freeze` | `dispute`.
    pub action: String,
    pub reason: String,
}

/// Register all `/case` routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/case", post(create_case).get(list_cases))
        .route(
            "/case/:id",
            get(get_case).patch(transition_case).delete(delete_case),
        )
        .route(
            "/case/:id/fact",
            post(crate::routes::fact::create_fact).get(crate::routes::fact::list_facts),
        )
        .route(
            "/case/:id/evidence",
            post(crate::routes::evidence::upload_evidence),
        )
        .route(
            "/case/:id/document",
            post(crate::routes::document::create_document),
        )
}

async fn create_case(State(s): State<AppState>, Json(req): Json<CreateCaseReq>) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };

    // Freeze the KB hash from the live KB version (INV-04) and refuse a Level-4-expired KB.
    let (Some(manifest), Some(version)) = (s.kb_manifest(), s.kb_version()) else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if kb::ensure_calculable(manifest.generated_at, Utc::now()).is_err() {
        return responses::error(data_model::ErrorCode::KbOutdated, trace);
    }

    // INV-04 server-authoritative freeze (§1.3.1): the FROZEN kb_version_hash is always derived on
    // the server. If the client supplied a non-empty hash that matches the live manifest's
    // global_hash (the only known KB version in R1a), we honour it (it is the same value); any
    // other value (stale / unknown / empty) is overridden by the live manifest hash. The client
    // never dictates the freeze — it can only confirm the version it believes it is on.
    let frozen_kb_hash = if !req.kb_version_hash.is_empty()
        && req.kb_version_hash == manifest.global_hash
    {
        req.kb_version_hash.clone()
    } else {
        manifest.global_hash.clone()
    };

    // Run the deterministic M1 diagnosis engine when a completed 问诊树 path is supplied. A
    // determinate result fills the REAL LD-NN-NN subcategory + coverage tier and opens the case as
    // `diagnosed`; an absent / incomplete / abstaining path keeps the `LD-00-00` placeholder + draft
    // (the engine never guesses — C-C-6 / prd §4.1.5). The diagnosis is otherwise run later via
    // POST /diagnose. The diagnosed flag is carried out for the audit record below.
    let mut diagnosed: Option<(String, CoverageTier, CoverageTag, f32)> = None;
    if !req.answer_path.is_empty() {
        if let Some(engine) = s.diagnosis_engine() {
            let input = rule_engine::DiagnosisInput {
                identity_type: req.identity_type,
                dispute_subtype: req.dispute_subtype,
                answer_path: req.answer_path.clone(),
                skipped_questions: req.skipped_questions,
            };
            if let rule_engine::DiagnosisOutput::Ok(r) = engine.diagnose(&input) {
                diagnosed = Some((
                    r.dispute_category.clone(),
                    r.coverage_tier,
                    r.coverage_tag,
                    r.confidence,
                ));
            }
        }
    }

    let now = Utc::now();
    let (dispute_category, coverage_tier, status) = match &diagnosed {
        Some((code, tier, _, _)) => (code.clone(), *tier, CaseStatus::Diagnosed),
        // No diagnosis at creation: placeholder (well-formed LD-NN-NN as the schema requires,
        // surfaced as `disputeCategoryId: null`) + draft.
        None => (
            UNDIAGNOSED_CATEGORY.to_string(),
            CoverageTier::default(),
            CaseStatus::Draft,
        ),
    };
    let case = Case {
        id: new_id(),
        identity_type: req.identity_type,
        dispute_subtype: req.dispute_subtype,
        dispute_category,
        coverage_tier,
        case_occurred_at: req.case_occurred_at,
        province: req.province,
        city: req.city,
        region_code: req.region_code,
        kb_version_hash: frozen_kb_hash,
        kb_version_label: version.version_label.clone(),
        status,
        group_id: None,
        dialogue_template_id: None,
        created_at: now,
        updated_at: now,
        frozen_at: None,
    };

    if let Err(e) = store.case.insert(&case).await {
        return responses::from_db_error(&e, trace);
    }

    // Audit: case creation is a state change (independent audit.sqlite, D3 / INV-06).
    if let Err(resp) = crate::audit_helper::append_state_change(
        &s,
        audit::AuditReason::CaseStatusTransition,
        case.id,
        serde_json::json!({
            "case_id": case.id.to_string(),
            "action": "create",
            "status": serde_json::to_value(case.status).unwrap_or(serde_json::Value::Null),
        }),
        &trace,
    )
    .await
    {
        return resp;
    }

    // When M1 diagnosis ran at creation, audit the deterministic diagnosis output (INV-06 mandatory
    // AI key-suggestion record; the engine is rule-based, recorded as `ai_diagnosis_output` so the
    // diagnosis surface has a uniform audit trail whether it runs here or via POST /diagnose).
    if let Some((code, tier, tag, confidence)) = &diagnosed {
        if let Err(resp) = crate::audit_helper::append_state_change(
            &s,
            audit::AuditReason::AiDiagnosisOutput,
            case.id,
            serde_json::json!({
                "case_id": case.id.to_string(),
                "dispute_category": code,
                "coverage_tier": tier,
                "coverage_tag": tag,
                "confidence": confidence,
                "engine": "deterministic_decision_tree",
            }),
            &trace,
        )
        .await
        {
            return resp;
        }
    }

    responses::created(CaseDto::from_case(&case), trace)
}

async fn list_cases(State(s): State<AppState>) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    // List the active (non-soft-deleted) cases (backend/01 §1.2, <= 3 parallel). Soft-deleted
    // cases are excluded by the repo query (migration 0008).
    match store.case.list_active().await {
        Ok(cases) => {
            let dtos: Vec<CaseDto> = cases.iter().map(CaseDto::from_case).collect();
            responses::ok_200(
                serde_json::json!({ "count": dtos.len(), "cases": dtos }),
                trace,
            )
        }
        Err(e) => responses::from_db_error(&e, trace),
    }
}

async fn get_case(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(case_id) = uuid::Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    let case = match store.case.get(&case_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    };

    let facts = match store.fact.list_by_case(&case_id).await {
        Ok(fs) => fs.iter().map(FactDto::from_fact).collect(),
        Err(e) => return responses::from_db_error(&e, trace),
    };
    let evidences = match store.evidence.list_by_case(&case_id).await {
        Ok(es) => es.iter().map(EvidenceDto::from_evidence).collect(),
        Err(e) => return responses::from_db_error(&e, trace),
    };

    let agg = CaseAggregateDto {
        case: CaseDto::from_case(&case),
        facts,
        evidences,
        // documents / claims / deadlines are owned by their own modules (later subtasks); the
        // aggregate shape is contract-complete with empty arrays for now.
        documents: Vec::new(),
        claims: Vec::new(),
        deadlines: Vec::new(),
    };
    responses::ok_200(agg, trace)
}

async fn transition_case(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CaseTransitionReq>,
) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(case_id) = uuid::Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    let mut case = match store.case.get(&case_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    };

    let target = match req.action.as_str() {
        "confirm" => CaseStatus::Confirmed,
        "freeze" => CaseStatus::Frozen,
        "dispute" => CaseStatus::Disputed,
        _ => return responses::error(data_model::ErrorCode::BadRequest, trace),
    };

    // In-process state-machine guard (mirrors the SQL trigger whitelist).
    if validate_case_transition(case.status, target).is_err() {
        return responses::error(data_model::ErrorCode::InvalidTransition, trace);
    }

    let from = case.status;
    case.status = target;
    case.updated_at = Utc::now();
    if target == CaseStatus::Frozen {
        case.frozen_at = Some(Utc::now());
    }

    if let Err(e) = store.case.update(&case).await {
        return responses::from_db_error(&e, trace);
    }

    if let Err(resp) = crate::audit_helper::append_state_change(
        &s,
        audit::AuditReason::CaseStatusTransition,
        case.id,
        serde_json::json!({
            "case_id": case.id.to_string(),
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

    responses::ok_200(CaseDto::from_case(&case), trace)
}

async fn delete_case(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(case_id) = uuid::Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    // backend/01 §1.2: DELETE /case/:id is a SOFT delete with a 30-day recycle bin — never a hard
    // delete (which FK RESTRICT would block while children exist anyway). The case is stamped
    // `deleted_at` (migration 0008); it is then excluded from GET /case + GET /case/:id (404).
    // `get` already excludes soft-deleted cases, so a second delete reads as not-found.
    match store.case.get(&case_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    }
    let deleted_at = Utc::now();
    match store.case.soft_delete(&case_id, &deleted_at).await {
        Ok(true) => {}
        Ok(false) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    }

    // Every mutation writes audit (D3): record the soft delete as a case state change.
    if let Err(resp) = crate::audit_helper::append_state_change(
        &s,
        audit::AuditReason::CaseStatusTransition,
        case_id,
        serde_json::json!({
            "case_id": case_id.to_string(),
            "action": "soft_delete",
            "deleted_at": deleted_at.to_rfc3339(),
            "recycle_days": 30,
        }),
        &trace,
    )
    .await
    {
        return resp;
    }

    responses::ok_200(serde_json::json!({ "deleted": true, "softDelete": true }), trace)
}
