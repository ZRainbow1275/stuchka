//! `/evidence` routes (backend/01 §1.2 / §1.5, module M2).
//!
//! `POST /case/:id/evidence` (multipart) handler lives here, registered under the case tree in
//! `routes/case.rs`; this module also owns `GET /evidence/:id` and `POST /evidence/:id/rescore`
//! plus the DTOs. The upload pipeline runs the **real** module crates (no stubs):
//! 1. `crates/hsd` scans the metadata text → if high-sensitive, the file is age-encrypted via
//!    `crypto::AgeStore` and the DB keeps the encrypted blob path; otherwise the raw file is
//!    written under `<data_dir>/evidence`;
//! 2. SHA-256 + byte size are computed;
//! 3. `crates/rule-engine` scores the five dimensions → `effective_score`;
//! 4. `crates/db` persists the `Evidence` row;
//! 5. `crates/audit` appends the upload (+ a high-sensitivity-detected record on a hit).

use axum::{
    extract::{Multipart, Path, State},
    response::Response,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use data_model::{new_id, Evidence, EvidenceCategory, EvidenceStatus, Timestamp};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{responses, state::AppState};

/// Evidence DTO (backend/01 §1.5.1). `id` / `caseId` carry UUID v7 36-char strings (D9).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDto {
    pub id: String,
    pub case_id: String,
    pub category: EvidenceCategory,
    pub status: EvidenceStatus,
    pub file_path: String,
    pub file_sha256: String,
    pub file_size_bytes: u64,
    pub is_high_sensitive: bool,
    pub pii_hits: Vec<PiiHit>,
    pub effective_score: f32,
    pub score_breakdown: ScoreBreakdown,
    pub linked_fact_ids: Vec<String>,
    pub gps: Option<GeoPoint>,
    pub collected_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

impl EvidenceDto {
    /// Project a persisted [`Evidence`] row into the DTO (pii_hits are derived at scan time and
    /// not persisted, so they are empty here; the upload response carries the live hits).
    pub fn from_evidence(e: &Evidence) -> Self {
        let sb = ScoreBreakdown::from_json(&e.score_breakdown);
        let gps = e
            .gps_coords
            .as_ref()
            .and_then(|v| serde_json::from_value::<GeoPoint>(v.clone()).ok());
        Self {
            id: e.id.to_string(),
            case_id: e.case_id.to_string(),
            category: e.evidence_type,
            status: e.status,
            file_path: e.file_path.clone(),
            file_sha256: e.file_hash.clone(),
            file_size_bytes: e.byte_size.max(0) as u64,
            is_high_sensitive: e.high_sensitivity,
            pii_hits: Vec::new(),
            effective_score: e.effective_score,
            score_breakdown: sb,
            linked_fact_ids: e.chain_membership.clone(),
            gps,
            collected_at: e.collected_at,
            created_at: e.created_at,
        }
    }
}

/// Five-dimension evidence score breakdown (backend/01 §1.5.1).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreBreakdown {
    pub source: f32,
    pub temporal: f32,
    pub integrity: f32,
    pub relevance: f32,
    pub authenticity: f32,
}

impl ScoreBreakdown {
    fn from_engine(b: &rule_engine::ScoreBreakdown) -> Self {
        Self {
            source: b.source,
            temporal: b.temporal,
            integrity: b.integrity,
            relevance: b.relevance,
            authenticity: b.authenticity,
        }
    }

    /// Persisted shape: the api DTO keys (`source/temporal/integrity/relevance/authenticity`).
    fn to_json(self) -> serde_json::Value {
        serde_json::json!({
            "source": self.source,
            "temporal": self.temporal,
            "integrity": self.integrity,
            "relevance": self.relevance,
            "authenticity": self.authenticity,
        })
    }

    fn from_json(v: &serde_json::Value) -> Self {
        let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
        Self {
            source: f("source"),
            temporal: f("temporal"),
            integrity: f("integrity"),
            relevance: f("relevance"),
            authenticity: f("authenticity"),
        }
    }
}

/// A high-sensitivity detector hit (`crates/hsd`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PiiHit {
    pub kind: String,
    pub span_start: usize,
    pub span_end: usize,
}

/// Geo point attached to evidence metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
    pub accuracy_m: f64,
}

/// The `metadata` JSON part of the multipart upload (backend/01 §1.5.1).
#[derive(Debug, Clone, Default, Deserialize)]
struct EvidenceMetadata {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    collected_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    gps: Option<GeoPoint>,
    #[serde(default)]
    linked_fact_ids: Vec<String>,
}

/// Register `/evidence/:id` and `/evidence/:id/rescore`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/evidence/:id", get(get_evidence))
        .route("/evidence/:id/rescore", post(rescore_evidence))
}

/// Map a metadata `category` string (or fall back to documentary) onto the seven-class enum.
fn parse_category(raw: Option<&str>) -> EvidenceCategory {
    match raw.unwrap_or("contract") {
        "contract" | "documentary" | "documentary_contract" => {
            EvidenceCategory::DocumentaryContract
        }
        "recording" | "audio" | "video" | "audio_video" => EvidenceCategory::AudioVideo,
        "chat" | "digital" | "digital_communication" => EvidenceCategory::DigitalCommunication,
        "witness" | "witness_statement" => EvidenceCategory::WitnessStatement,
        "photo" | "scene" | "scene_photo_video" => EvidenceCategory::ScenePhotoVideo,
        "third_party" | "third_party_data" => EvidenceCategory::ThirdPartyData,
        "appraisal" => EvidenceCategory::Appraisal,
        _ => EvidenceCategory::DocumentaryContract,
    }
}

/// `POST /case/:id/evidence` (multipart) — the real upload pipeline.
pub async fn upload_evidence(
    State(s): State<AppState>,
    Path(case_id): Path<String>,
    mut multipart: Multipart,
) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(case_uuid) = uuid::Uuid::parse_str(&case_id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };
    match store.case.get(&case_uuid).await {
        Ok(Some(_)) => {}
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    }

    // Drain the multipart: collect the file bytes + the metadata JSON.
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut metadata = EvidenceMetadata::default();
    let mut metadata_raw = String::new();
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                match name.as_str() {
                    "file" => match field.bytes().await {
                        Ok(b) => file_bytes = b.to_vec(),
                        Err(_) => {
                            return responses::error(data_model::ErrorCode::BadRequest, trace)
                        }
                    },
                    "metadata" => match field.text().await {
                        Ok(t) => {
                            metadata_raw = t.clone();
                            metadata = serde_json::from_str(&t).unwrap_or_default();
                        }
                        Err(_) => {
                            return responses::error(data_model::ErrorCode::BadRequest, trace)
                        }
                    },
                    _ => {
                        let _ = field.bytes().await;
                    }
                }
            }
            Ok(None) => break,
            Err(_) => return responses::error(data_model::ErrorCode::BadRequest, trace),
        }
    }

    if file_bytes.is_empty() {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    }

    // 1. High-sensitivity scan over the metadata text + file (text best-effort) — crates/hsd.
    let Some(hsd) = s.hsd() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let scan_text = {
        let mut t = metadata_raw.clone();
        if let Ok(file_text) = std::str::from_utf8(&file_bytes) {
            t.push('\n');
            t.push_str(file_text);
        }
        t
    };
    let report = hsd.scan(&scan_text);
    let pii_hits: Vec<PiiHit> = report
        .hits
        .iter()
        .map(|h| PiiHit {
            kind: serde_json::to_value(h.kind)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default(),
            span_start: h.span.start,
            span_end: h.span.end,
        })
        .collect();
    let high_sensitive = report.is_high_sensitive;

    // 2. SHA-256 + size.
    let file_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&file_bytes);
        format!("{:x}", hasher.finalize())
    };
    let byte_size = file_bytes.len() as i64;

    // High-sensitive → age-encrypt the file and store only the blob path; else store the raw file.
    let file_path = if high_sensitive {
        let Some(age) = s.age_store() else {
            return responses::error(data_model::ErrorCode::Internal, trace);
        };
        match age.write(&file_bytes) {
            Ok(blob) => blob.path,
            Err(e) => {
                tracing::error!(error = %e, "age encrypt failed");
                return responses::error(data_model::ErrorCode::Internal, trace);
            }
        }
    } else {
        let dir = std::path::Path::new("data").join("evidence");
        if let Err(e) = tokio::fs::create_dir_all(&dir).await {
            tracing::error!(error = %e, "create evidence dir failed");
            return responses::error(data_model::ErrorCode::Internal, trace);
        }
        let path = dir.join(format!("{}", new_id()));
        if let Err(e) = tokio::fs::write(&path, &file_bytes).await {
            tracing::error!(error = %e, "write evidence file failed");
            return responses::error(data_model::ErrorCode::Internal, trace);
        }
        path.to_string_lossy().into_owned()
    };

    // 3. Five-dimension score — crates/rule-engine.
    let category = parse_category(metadata.category.as_deref());
    let inputs = rule_engine::EvidenceScoreInputs {
        category,
        integrity_hash_present: true,
        has_collected_at: metadata.collected_at.is_some(),
        has_device_id: metadata.device_id.is_some(),
        has_gps: metadata.gps.is_some(),
        linked_fact_count: metadata.linked_fact_ids.len(),
    };
    let breakdown_engine = rule_engine::score_evidence(&inputs);
    let breakdown = ScoreBreakdown::from_engine(&breakdown_engine);
    let effective_score = breakdown_engine.effective_score();

    // 4. Persist the Evidence row — crates/db.
    let now = Utc::now();
    let evidence = Evidence {
        id: new_id(),
        case_id: case_uuid,
        evidence_type: category,
        file_path: file_path.clone(),
        file_hash: file_sha256.clone(),
        mime_type: "application/octet-stream".to_string(),
        byte_size,
        effective_score,
        score_breakdown: breakdown.to_json(),
        status: EvidenceStatus::Scored,
        high_sensitivity: high_sensitive,
        collected_at: metadata.collected_at,
        device_id: metadata.device_id.clone(),
        gps_coords: metadata
            .gps
            .as_ref()
            .and_then(|g| serde_json::to_value(g).ok()),
        chain_membership: metadata.linked_fact_ids.clone(),
        created_at: now,
        updated_at: now,
    };
    if let Err(e) = store.evidence.insert(&evidence).await {
        return responses::from_db_error(&e, trace);
    }

    // 5. Audit: evidence upload (+ high-sensitivity detected on a hit).
    if let Err(resp) = crate::audit_helper::append(
        &s,
        audit::Subject::System {
            component: "evidence".to_string(),
        },
        audit::AuditReason::EvidenceUploaded,
        case_uuid,
        serde_json::json!({
            "case_id": case_uuid.to_string(),
            "evidence_id": evidence.id.to_string(),
            "sha256": file_sha256,
            "high_sensitive": high_sensitive,
        }),
        &trace,
    )
    .await
    {
        return resp;
    }
    if high_sensitive {
        if let Err(resp) = crate::audit_helper::append(
            &s,
            audit::Subject::System {
                component: "hsd".to_string(),
            },
            audit::AuditReason::HighSensitivityDetected,
            case_uuid,
            serde_json::json!({
                "case_id": case_uuid.to_string(),
                "evidence_id": evidence.id.to_string(),
                "hit_count": pii_hits.len(),
            }),
            &trace,
        )
        .await
        {
            return resp;
        }
    }

    let mut dto = EvidenceDto::from_evidence(&evidence);
    dto.pii_hits = pii_hits;
    responses::created(dto, trace)
}

async fn get_evidence(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(ev_id) = uuid::Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };
    match store.evidence.get(&ev_id).await {
        Ok(Some(e)) => responses::ok_200(EvidenceDto::from_evidence(&e), trace),
        Ok(None) => responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => responses::from_db_error(&e, trace),
    }
}

async fn rescore_evidence(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let trace = s.new_trace_id();
    let Some(store) = s.store() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(ev_id) = uuid::Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    let mut evidence = match store.evidence.get(&ev_id).await {
        Ok(Some(e)) => e,
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    };

    let inputs = rule_engine::EvidenceScoreInputs {
        category: evidence.evidence_type,
        integrity_hash_present: !evidence.file_hash.is_empty(),
        has_collected_at: evidence.collected_at.is_some(),
        has_device_id: evidence.device_id.is_some(),
        has_gps: evidence.gps_coords.is_some(),
        linked_fact_count: evidence.chain_membership.len(),
    };
    let breakdown_engine = rule_engine::score_evidence(&inputs);
    let breakdown = ScoreBreakdown::from_engine(&breakdown_engine);
    evidence.effective_score = breakdown_engine.effective_score();
    evidence.score_breakdown = breakdown.to_json();
    evidence.updated_at = Utc::now();

    if let Err(e) = store.evidence.update(&evidence).await {
        return responses::from_db_error(&e, trace);
    }

    if let Err(resp) = crate::audit_helper::append(
        &s,
        audit::Subject::RuleEngine {
            module: "evidence_score".to_string(),
            version: "1".to_string(),
        },
        audit::AuditReason::EvidenceScored,
        evidence.case_id,
        serde_json::json!({
            "case_id": evidence.case_id.to_string(),
            "evidence_id": evidence.id.to_string(),
            "effective_score": evidence.effective_score,
        }),
        &trace,
    )
    .await
    {
        return resp;
    }

    responses::ok_200(EvidenceDto::from_evidence(&evidence), trace)
}
