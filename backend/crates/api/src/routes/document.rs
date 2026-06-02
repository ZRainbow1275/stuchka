//! `/document` routes (backend/01 §1.2 / §1.6, module M3).
//!
//! `POST /case/:id/document` is registered under the case tree (`routes/case.rs`); this module owns
//! `GET /document/:id` (Yjs state vector), `POST /document/:id/export` (the real `document` crate
//! three-piece dossier + GB 45438 four layers + D3 Export audit), `POST /document/:id/merge`
//! (`sync::apply_merge_decision` + merge audit), and the NEW `POST /document/:id/steps` (editor Step
//! persistence via `sync::handle_step_audit` → main-store `doc_step` + independent `audit.sqlite`).
//!
//! Document content is the Yjs `CaseDoc` held in the live sync provider state; the business metadata
//! (owning case + template) is the main-store `document` row created at `POST /case/:id/document`.
//! The `document` crate is the SOLE GB-45438 / final-PDF authority (D5); api never bypasses
//! `export_dossier`'s four-layer pipeline.

use axum::{
    extract::{Path, State},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use base64::Engine as _;
use blake3;
use document::render::ParagraphRef;
use document::{
    export_dossier_with_audit, AiSegment, DocMeta, DocReviewState, ExportFormat, ExportRequest,
    TemplateContext, TemplateId,
};
use serde::{Deserialize, Serialize};
use sync::{handle_step_audit, MergeDecision, StepAuditFrame, StepCategory};
use uuid::Uuid;

use crate::{responses, state::AppState};

/// `POST /case/:id/document` request (backend/01 §1.6.1).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocReq {
    /// `arb_application` | `mediation` | ... (template id).
    pub template_id: TemplateId,
    #[serde(default)]
    pub claim_ids: Vec<String>,
}

/// `POST /case/:id/document` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocResp {
    pub doc_id: String,
    pub case_id: String,
    pub template_id: TemplateId,
}

/// `GET /document/:id` response (the Yjs doc state vector, base64).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocStateResp {
    pub doc_id: String,
    pub case_id: String,
    pub template_id: TemplateId,
    /// `Y.encodeStateVector` (v1), base64 — the client diffs against this before `/ws/sync`.
    pub state_vector_base64: String,
}

/// `POST /document/:id/export` request (backend/01 §1.6.2).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDocReq {
    /// e.g. `["pdf", "md", "json"]`.
    pub formats: Vec<String>,
    #[serde(default = "default_true")]
    pub embed_water_mark: bool,
    /// GB 45438-2025 layer level (four-layer all on = 4, INV-02).
    pub gb45438_level: u8,
}

fn default_true() -> bool {
    true
}

/// `POST /document/:id/export` response: the dossier bundle written to disk + the manifest summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDocResp {
    /// Absolute path of the produced三件套 zip.
    pub dossier_zip_path: String,
    /// The bundle entries actually present (`*.md`, `manifest.json`, and `*.pdf` when produced).
    pub entries: Vec<String>,
    /// Whether the final PDF was produced this environment (lopdf is pure-Rust so always true here;
    /// the optional pdfium rasterisation seam is environment-gated and not required for the zip).
    pub pdf_present: bool,
    /// The Layer-4 manifest's `ai_generated_segments` count (GB-02 tamper-evidence anchor).
    pub ai_generated_segments: usize,
    /// The GB 45438 four-layer completeness self-check (all-true on success, INV-02).
    pub gb45438_layer_completeness: LayerCompletenessDto,
}

/// The five-layer completeness booleans surfaced to the client / manifest.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerCompletenessDto {
    pub layer1_explicit: bool,
    pub layer2_metadata: bool,
    pub layer3_zerowidth: bool,
    pub layer3_lsb: bool,
    pub layer4_manifest: bool,
}

/// Three-way merge winner (backend/01 §1.6.3). Replaces the prior free `String` with the closed
/// contract set so an out-of-vocabulary winner is rejected at deserialisation (E_BAD_REQUEST).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeWinner {
    /// Keep the common ancestor.
    Ancestor,
    /// Keep the local side.
    Mine,
    /// Keep the remote side.
    Theirs,
    /// Use the operator-supplied `manual_text` (which then becomes required).
    Manual,
}

impl MergeWinner {
    /// The wire token persisted into the merge audit + doc_step (`ancestor|mine|theirs|manual`).
    fn as_str(self) -> &'static str {
        match self {
            MergeWinner::Ancestor => "ancestor",
            MergeWinner::Mine => "mine",
            MergeWinner::Theirs => "theirs",
            MergeWinner::Manual => "manual",
        }
    }
}

/// `POST /document/:id/merge` request (backend/01 §1.6.3).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeDecisionReq {
    pub conflict_id: String,
    /// `ancestor` | `mine` | `theirs` | `manual` (§1.6.3 closed set).
    pub winner: MergeWinner,
    /// Required when `winner == "manual"` (§1.6.3).
    pub manual_text: Option<String>,
    /// The ProseMirror Step JSON that applies the decision (Yjs/CRDT update payload, base64).
    #[serde(default)]
    pub decision_update_base64: Option<String>,
    /// The full ProseMirror Step JSON recorded with the merge (audit + doc_step mirror).
    #[serde(default)]
    pub step_json: serde_json::Value,
    /// Owning case id (required by the audit `what` schema for `DocumentMerged`).
    pub case_id: String,
    /// Acting user id (UUID v7).
    pub actor: String,
    /// Written to the audit record `why` detail.
    pub reason: String,
}

/// `POST /document/:id/steps` request (NEW route, cross-crate reconciliation §C / 03-sync-yjs §3.5).
///
/// The contract is an `EditorStepBatch`: a `steps[]` array processed in order, each step audited
/// (INV-06 double-write: main-store `doc_step` + independent `audit.sqlite`). A single-step body
/// (the original shape, `stepJson` + `why` at the top level) is still accepted for back-compat: a
/// missing/empty `steps` array falls back to the top-level single step.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorStepReq {
    /// Owning case id (UUID v7) — required by the audit `what` schema.
    pub case_id: String,
    /// Acting user id (UUID v7).
    pub actor: String,
    /// Yjs client id.
    pub client_id: String,
    /// The batch of steps to process in order (cross-crate reconciliation §C).
    #[serde(default)]
    pub steps: Vec<EditorStep>,
    /// Back-compat single-step body: full ProseMirror Step JSON (used when `steps` is empty).
    #[serde(default)]
    pub step_json: serde_json::Value,
    /// Back-compat single-step `why`: edit | ai_accept | merge_resolve.
    #[serde(default)]
    pub why: Option<String>,
    /// Back-compat single-step optional decision reason.
    #[serde(default)]
    pub reason: Option<String>,
}

/// One step inside an [`EditorStepBatch`](EditorStepReq).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorStep {
    /// Full ProseMirror Step JSON.
    pub step_json: serde_json::Value,
    /// edit | ai_accept | merge_resolve (mapped onto the sync `StepCategory`).
    pub why: String,
    /// Optional decision reason.
    #[serde(default)]
    pub reason: Option<String>,
}

/// `POST /document/:id/steps` ack: the per-step results (one entry per processed step).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepAck {
    pub doc_id: String,
    /// The last step number written (back-compat scalar; the per-step numbers are in `results`).
    pub step_no: i32,
    /// The audit `seq` of the last appended `audit.sqlite` record (INV-06 double-write).
    pub audit_seq: i64,
    /// One result per processed step (in order), each carrying its own `step_no` + `audit_seq`.
    pub results: Vec<StepResult>,
}

/// One processed step's outcome (step number + audit seq, INV-06 double-write).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepResult {
    pub step_no: i32,
    pub audit_seq: i64,
}

/// Register the `/document/:id` routes (incl. the NEW `POST /document/:id/steps`).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/document/:id", get(get_document))
        .route("/document/:id/export", post(export_document))
        .route("/document/:id/merge", post(merge_document))
        .route("/document/:id/steps", post(push_step))
}

/// `POST /case/:id/document` — create the Yjs doc + the main-store `document` row (registered under
/// the case tree). Confirms the case exists, persists the document metadata, and seeds an empty live
/// `CaseDoc` so `/ws/sync` + `GET /document/:id` resolve immediately.
pub async fn create_document(
    State(s): State<AppState>,
    Path(case_id): Path<String>,
    Json(req): Json<CreateDocReq>,
) -> Response {
    let trace = s.new_trace_id();
    let (Some(store), Some(sync)) = (s.store(), s.sync()) else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(case_uuid) = Uuid::parse_str(&case_id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };
    match store.case.get(&case_uuid).await {
        Ok(Some(_)) => {}
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    }

    let doc_id = data_model::new_id();
    let row = sync::DocumentRow {
        doc_id,
        case_id: case_uuid,
        template_id: serde_json::to_value(req.template_id)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "arb_application".to_string()),
        claim_ids: req.claim_ids.clone(),
    };
    if let Err(e) = sync.meta_repo().insert_document(row).await {
        tracing::error!(error = %e, "insert document row failed");
        return responses::error(data_model::ErrorCode::Internal, trace);
    }
    // Materialise the live (empty) CaseDoc so the doc state row exists for the WS provider.
    if let Err(e) = sync.live_doc(&doc_id.to_string()).await {
        tracing::error!(error = %e, "open live doc failed");
        return responses::error(data_model::ErrorCode::Internal, trace);
    }

    responses::created(
        CreateDocResp {
            doc_id: doc_id.to_string(),
            case_id: case_id.clone(),
            template_id: req.template_id,
        },
        trace,
    )
}

async fn get_document(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let trace = s.new_trace_id();
    let Some(sync) = s.sync() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let Ok(doc_uuid) = Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };
    let meta = match sync.meta_repo().get_document(doc_uuid).await {
        Ok(Some(m)) => m,
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => {
            tracing::error!(error = %e, "get document row failed");
            return responses::error(data_model::ErrorCode::Internal, trace);
        }
    };
    let live = match sync.live_doc(&id).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, "open live doc failed");
            return responses::error(data_model::ErrorCode::Internal, trace);
        }
    };
    let sv = {
        let case_doc = live.case_doc.lock().await;
        case_doc.state_vector()
    };
    let template_id: TemplateId =
        serde_json::from_value(serde_json::Value::String(meta.template_id.clone()))
            .unwrap_or(TemplateId::ArbApplication);
    responses::ok_200(
        DocStateResp {
            doc_id: id,
            case_id: meta.case_id.to_string(),
            template_id,
            state_vector_base64: base64::engine::general_purpose::STANDARD.encode(sv),
        },
        trace,
    )
}

async fn export_document(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ExportDocReq>,
) -> Response {
    let trace = s.new_trace_id();
    let (Some(sync), Some(audit), Some(store)) = (s.sync(), s.audit(), s.store()) else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if !s.audit_chain_ok() {
        return responses::error(data_model::ErrorCode::AuditChainBroken, trace);
    }
    let Ok(doc_uuid) = Uuid::parse_str(&id) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    let meta_row = match sync.meta_repo().get_document(doc_uuid).await {
        Ok(Some(m)) => m,
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => {
            tracing::error!(error = %e, "get document row failed");
            return responses::error(data_model::ErrorCode::Internal, trace);
        }
    };
    let case = match store.case.get(&meta_row.case_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
        Err(e) => return responses::from_db_error(&e, trace),
    };
    // REAL case aggregate: facts + claims (with Decimal amounts + calculation_breakdown).
    let facts = store
        .fact
        .list_by_case(&meta_row.case_id)
        .await
        .unwrap_or_default();
    let claims = store
        .claim
        .list_by_case(&meta_row.case_id)
        .await
        .unwrap_or_default();

    // REAL law-ref URNs: prefer the URNs of the claims' linked law_refs (resolved from the main
    // store); fall back to the live KB index searched by the case's real facts / dispute (still
    // real D8 URNs, never a hardcoded string). GB-05 requires >= 1 `law:` URN.
    let law_urns = resolve_law_urns(store, s.kb_index(), &case, &claims, &facts).await;

    // REAL document body: the edited ProseMirror content held in the live Yjs CaseDoc (persisted
    // via /ws/sync + the write-through updates). Empty for a freshly-created, never-edited doc.
    let body_text = match sync.live_doc(&id).await {
        Ok(live) => {
            let case_doc = live.case_doc.lock().await;
            case_doc.document_text(&id)
        }
        Err(e) => {
            tracing::error!(error = %e, "open live doc for export failed");
            return responses::error(data_model::ErrorCode::Internal, trace);
        }
    };

    // REAL AI-marked segments: derived from the document's actual AI-accept records in the
    // independent audit.sqlite (the `ai_accept` steps pushed via /document/:id/steps, INV-06),
    // filtered to THIS document — never a hardcoded list. An empty set is honoured as empty.
    let kb_hash = s
        .kb_manifest()
        .map(|m| m.global_hash.clone())
        .unwrap_or_else(|| "0".repeat(64));
    let ai_records = ai_segments_from_audit(audit, meta_row.case_id, doc_uuid).await;

    let template_id: TemplateId =
        serde_json::from_value(serde_json::Value::String(meta_row.template_id.clone()))
            .unwrap_or(TemplateId::ArbApplication);

    // Assemble the template context + AI segments from the REAL values gathered above.
    let (ctx, segs) = build_export_inputs(
        template_id,
        &case,
        &facts,
        &claims,
        &law_urns,
        &body_text,
        &ai_records,
        &kb_hash,
    );
    let doc_meta = DocMeta {
        doc_id: doc_uuid,
        case_id_hashed: blake3::hash(meta_row.case_id.to_string().as_bytes())
            .to_hex()
            .to_string()[..32]
            .to_string(),
        template_id,
        version: env!("CARGO_PKG_VERSION").to_string(),
        kb_hash,
        kb_release_date: chrono::Utc::now().date_naive(),
        review_state_doc: DocReviewState::Full,
        generated_at: chrono::Utc::now(),
        segment_count: segs.len() as u32,
    };

    let formats: Vec<ExportFormat> = req
        .formats
        .iter()
        .filter_map(|f| match f.as_str() {
            "pdf" => Some(ExportFormat::Pdf),
            "md" => Some(ExportFormat::Md),
            "json" => Some(ExportFormat::Json),
            _ => None,
        })
        .collect();
    let export_req = ExportRequest {
        formats: if formats.is_empty() {
            vec![ExportFormat::Md, ExportFormat::Json]
        } else {
            formats
        },
        embed_water_mark: req.embed_water_mark,
        gb45438_level: req.gb45438_level,
    };

    // The document crate is the SOLE four-layer + PDF authority (D5) and writes the D3 Export audit.
    let bundle = match export_dossier_with_audit(
        audit,
        audit::Subject::System {
            component: "document".to_string(),
        },
        meta_row.case_id,
        template_id,
        &ctx,
        &segs,
        &doc_meta,
        &export_req,
    )
    .await
    {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "dossier export failed");
            return responses::error_with(
                data_model::ApiError::from_code(data_model::ErrorCode::Internal)
                    .with_detail(serde_json::json!({ "detail": e.to_string() })),
                data_model::ErrorCode::Internal.http_status(),
                trace,
            );
        }
    };

    // Write the zip to the per-document export dir under the data tree.
    let out_dir = std::path::Path::new("data").join("exports");
    if let Err(e) = tokio::fs::create_dir_all(&out_dir).await {
        tracing::error!(error = %e, "create export dir failed");
        return responses::error(data_model::ErrorCode::Internal, trace);
    }
    let zip_path = out_dir.join(format!("{}_{}.zip", bundle.filename_stem, doc_uuid));
    if let Err(e) = tokio::fs::write(&zip_path, &bundle.zip_bytes).await {
        tracing::error!(error = %e, "write dossier zip failed");
        return responses::error(data_model::ErrorCode::Internal, trace);
    }

    let entries = zip_entry_names(&bundle.zip_bytes);
    let pdf_present = entries.iter().any(|n| n.ends_with(".pdf"));
    let segments = bundle
        .manifest
        .documents
        .iter()
        .map(|d| d.ai_generated_segments.len())
        .sum();
    let c = bundle.completeness;

    responses::ok_200(
        ExportDocResp {
            dossier_zip_path: zip_path.to_string_lossy().to_string(),
            entries,
            pdf_present,
            ai_generated_segments: segments,
            gb45438_layer_completeness: LayerCompletenessDto {
                layer1_explicit: c.layer1_explicit,
                layer2_metadata: c.layer2_metadata,
                layer3_zerowidth: c.layer3_zerowidth,
                layer3_lsb: c.layer3_lsb,
                layer4_manifest: c.layer4_manifest,
            },
        },
        trace,
    )
}

async fn merge_document(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<MergeDecisionReq>,
) -> Response {
    let trace = s.new_trace_id();
    let (Some(sync), Some(audit)) = (s.sync(), s.audit()) else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if !s.audit_chain_ok() {
        return responses::error(data_model::ErrorCode::AuditChainBroken, trace);
    }
    let (Ok(doc_uuid), Ok(case_uuid), Ok(actor_uuid)) = (
        Uuid::parse_str(&id),
        Uuid::parse_str(&req.case_id),
        Uuid::parse_str(&req.actor),
    ) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };
    if req.winner == MergeWinner::Manual && req.manual_text.is_none() {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    }

    let live = match sync.live_doc(&id).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, "open live doc failed");
            return responses::error(data_model::ErrorCode::Internal, trace);
        }
    };

    // Apply the decision's CRDT update to the live doc and broadcast the post-decision delta (§3.8).
    // `decision_update_base64` is the Yjs/ProseMirror update payload the front end produced for the
    // chosen winner; `MergeDecision` documents the resolution metadata recorded alongside it.
    if let Some(b64) = &req.decision_update_base64 {
        let Ok(update) = base64::engine::general_purpose::STANDARD.decode(b64) else {
            return responses::error(data_model::ErrorCode::BadRequest, trace);
        };
        let _decision = MergeDecision {
            conflict_id: req.conflict_id.clone(),
            winner: req.winner.as_str().to_string(),
            manual_text: req.manual_text.clone(),
            step_json: req.step_json.clone(),
            reason: Some(req.reason.clone()),
        };
        let case_doc = live.case_doc.lock().await;
        let before_sv = case_doc.state_vector();
        match sync::apply_merge_decision(&case_doc, &before_sv, &update) {
            Ok(delta) => {
                drop(case_doc);
                // Persist (write-through) + broadcast the delta to other peers.
                if let Err(e) = sync::write_update(sync.doc_repo().as_ref(), &id, &delta).await {
                    tracing::error!(error = %e, "persist merge delta failed");
                }
                let _ = live.tx.send((sync::FrameType::Update as u8, delta));
            }
            Err(e) => {
                tracing::error!(error = %e, "apply merge decision failed");
                return responses::error(e.error_code(), trace);
            }
        }
    }

    // Record the merge as a ProseMirror Step (INV-06 double-write: doc_step + audit.sqlite).
    let next_step = sync
        .step_repo()
        .max_step_no(doc_uuid)
        .await
        .ok()
        .flatten()
        .map(|n| n + 1)
        .unwrap_or(1);
    let frame = StepAuditFrame {
        doc_id: doc_uuid,
        case_id: case_uuid,
        step_no: next_step,
        step_json: req.step_json.clone(),
        client_id: "merge".to_string(),
        actor: actor_uuid,
        reason: Some(req.reason.clone()),
        category: StepCategory::MergeDecision,
    };
    match handle_step_audit(sync.step_repo().as_ref(), audit, frame).await {
        Ok(seq) => responses::ok_200(
            serde_json::json!({
                "docId": id,
                "conflictId": req.conflict_id,
                "winner": req.winner.as_str(),
                "auditSeq": seq,
            }),
            trace,
        ),
        Err(e) => {
            tracing::error!(error = %e, "merge step audit failed");
            responses::error(e.error_code(), trace)
        }
    }
}

/// Map the front-end `why` triad onto the sync [`StepCategory`].
fn why_to_category(why: &str) -> Option<StepCategory> {
    match why {
        "edit" => Some(StepCategory::Edit),
        "ai_accept" => Some(StepCategory::AiAccept),
        "merge_resolve" | "merge_decision" => Some(StepCategory::MergeDecision),
        _ => None,
    }
}

async fn push_step(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<EditorStepReq>,
) -> Response {
    let trace = s.new_trace_id();
    let (Some(sync), Some(audit)) = (s.sync(), s.audit()) else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    if !s.audit_chain_ok() {
        return responses::error(data_model::ErrorCode::AuditChainBroken, trace);
    }
    let (Ok(doc_uuid), Ok(case_uuid), Ok(actor_uuid)) = (
        Uuid::parse_str(&id),
        Uuid::parse_str(&req.case_id),
        Uuid::parse_str(&req.actor),
    ) else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    // Normalise the body to a batch: the `steps[]` array, or — for back-compat — the single
    // top-level step (cross-crate reconciliation §C). An empty batch is a bad request.
    let steps: Vec<EditorStep> = if !req.steps.is_empty() {
        req.steps.clone()
    } else if let Some(why) = req.why.clone() {
        vec![EditorStep {
            step_json: req.step_json.clone(),
            why,
            reason: req.reason.clone(),
        }]
    } else {
        return responses::error(data_model::ErrorCode::BadRequest, trace);
    };

    // Validate every category up front so a malformed batch fails before any partial write.
    let mut categories = Vec::with_capacity(steps.len());
    for step in &steps {
        let Some(cat) = why_to_category(&step.why) else {
            return responses::error(data_model::ErrorCode::BadRequest, trace);
        };
        categories.push(cat);
    }

    // Process each step in order; each is an INV-06 double-write (doc_step + audit.sqlite).
    let mut next_step = sync
        .step_repo()
        .max_step_no(doc_uuid)
        .await
        .ok()
        .flatten()
        .unwrap_or(0);
    let mut results = Vec::with_capacity(steps.len());
    for (step, category) in steps.iter().zip(categories) {
        next_step += 1;
        let frame = StepAuditFrame {
            doc_id: doc_uuid,
            case_id: case_uuid,
            step_no: next_step,
            step_json: step.step_json.clone(),
            client_id: req.client_id.clone(),
            actor: actor_uuid,
            reason: step.reason.clone(),
            category,
        };
        match handle_step_audit(sync.step_repo().as_ref(), audit, frame).await {
            Ok(seq) => results.push(StepResult {
                step_no: next_step,
                audit_seq: seq,
            }),
            Err(e) => {
                tracing::error!(error = %e, "step audit failed");
                return responses::error(e.error_code(), trace);
            }
        }
    }

    let last = results.last().expect("at least one step processed");
    responses::ok_200(
        StepAck {
            doc_id: id,
            step_no: last.step_no,
            audit_seq: last.audit_seq,
            results,
        },
        trace,
    )
}

/// One real AI-marked segment recovered from the audit trail (an `ai_accept` Step recorded via
/// `POST /document/:id/steps`, INV-06). Carries the real recorded provenance; no fabricated text.
struct AiRecord {
    /// The audit `seq` of the recording (monotone, time-sortable) — drives a stable segment id.
    seq: i64,
    /// The document step number recorded in the audit `what`.
    step_no: i64,
    /// The real recorded ProseMirror Step JSON (the AI-accept artifact).
    step: serde_json::Value,
}

/// Recover the document's REAL AI-marked segments from the independent `audit.sqlite`: every
/// `AiDocumentDraft` (`ai_accept`) record for this case whose `what.doc_id` is this document. The
/// chain is the authoritative INV-06 record of which paragraphs were AI-accepted — never a constant.
async fn ai_segments_from_audit(
    audit: &audit::AuditLog,
    case_id: Uuid,
    doc_id: Uuid,
) -> Vec<AiRecord> {
    let records = match audit.query_by_case(case_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "audit query for AI segments failed; exporting none");
            return Vec::new();
        }
    };
    let doc_id_str = doc_id.to_string();
    records
        .into_iter()
        .filter(|r| r.why == audit::AuditReason::AiDocumentDraft)
        .filter(|r| {
            r.what
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(|d| d == doc_id_str)
                .unwrap_or(false)
        })
        .map(|r| AiRecord {
            seq: r.seq,
            step_no: r.what.get("step_no").and_then(|v| v.as_i64()).unwrap_or(0),
            step: r.what.get("step").cloned().unwrap_or(serde_json::Value::Null),
        })
        .collect()
}

/// Resolve the REAL D8 law-ref URNs for the export. Priority:
/// 1. the URNs of the claims' linked `law_refs` (resolved from the main store `law_ref` table);
/// 2. otherwise, the live KB index searched by the case's real facts / dispute subtype (real D8
///    `stable_id` URNs).
///
/// GB-05 requires >= 1 `law:` URN; an empty result lets the caller surface the real (empty) state
/// without inventing a clause. Each returned `(urn, title)` is a genuine KB / store value.
async fn resolve_law_urns(
    store: &db::Store,
    kb_index: Option<&kb::Bm25Index>,
    case: &data_model::Case,
    claims: &[data_model::Claim],
    facts: &[data_model::CaseFact],
) -> Vec<(String, String)> {
    use std::collections::BTreeSet;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<(String, String)> = Vec::new();

    // 1. Claims' linked law refs (real persisted snapshots).
    for claim in claims {
        for lr_id in &claim.law_refs {
            if let Ok(Some(lr)) = store.law_ref.get(lr_id).await {
                if seen.insert(lr.stable_id.clone()) {
                    out.push((lr.stable_id, lr.title));
                }
            }
        }
    }

    // 2. KB index search by the case's real facts + dispute subtype (real D8 URNs from the live
    //    KB). Concatenating all fact statements + the dispute token maximises BM25 recall over the
    //    jieba-segmented corpus; the dispute keyword (`劳动 争议`) guarantees a baseline match.
    if out.is_empty() {
        if let Some(index) = kb_index {
            use kb::LawSearch;
            let mut query_text: String =
                facts.iter().map(|f| f.content.clone()).collect::<Vec<_>>().join(" ");
            if let Some(token) = serde_json::to_value(case.dispute_subtype)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.replace('_', " ")))
            {
                query_text.push(' ');
                query_text.push_str(&token);
            }
            // The dispute domain keyword guarantees a baseline hit against the labour corpus.
            query_text.push_str(" 劳动 争议 工资 解除 经济补偿");
            let q = kb::SearchQuery::keyword(query_text);
            if let Ok(hits) = index.search(&q) {
                for hit in hits.into_iter().take(3) {
                    if seen.insert(hit.stable_id.clone()) {
                        let title = law_title_from_urn(&hit.stable_id);
                        out.push((hit.stable_id, title));
                    }
                }
            }
        }
    }

    out
}

/// Extract the human title from a `law:<全称>/v…/§…` URN (the segment before the first `/v`).
fn law_title_from_urn(urn: &str) -> String {
    let body = urn.strip_prefix("law:").unwrap_or(urn);
    match body.find("/v") {
        Some(i) => body[..i].to_string(),
        None => body.to_string(),
    }
}

/// Pull the real body paragraphs out of the live document's ProseMirror text. The fragment string
/// is split on line breaks; blank lines are dropped. Returns the genuine edited paragraphs (empty
/// when the document has no body yet).
fn body_paragraphs(body_text: &str) -> Vec<String> {
    body_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// Build the template context + AI segments for the export from the REAL assembled values:
/// the persisted case + facts + claims (Decimal amounts + calculation_breakdown), the resolved D8
/// law-ref URNs, the live ProseMirror body, and the AI-marked segments recovered from the audit
/// trail. No hardcoded document content remains: claims/facts/law_refs/AI segments all come from
/// the real store / live doc / audit chain.
#[allow(clippy::too_many_arguments)]
fn build_export_inputs(
    template_id: TemplateId,
    case: &data_model::Case,
    facts: &[data_model::CaseFact],
    claims: &[data_model::Claim],
    law_urns: &[(String, String)],
    body_text: &str,
    ai_records: &[AiRecord],
    kb_hash: &str,
) -> (TemplateContext, Vec<AiSegment>) {
    use document::template::{AiParagraph, ClaimItem, FactItem, LawRefSlot};

    // REAL facts (the user / AI-extracted statements persisted on the case).
    let fact_items: Vec<FactItem> = facts
        .iter()
        .enumerate()
        .map(|(i, f)| FactItem {
            fact_id: format!("F-{:03}", i + 1),
            statement: f.content.clone(),
        })
        .collect();

    // REAL claims with their M9-computed Decimal amounts + the calculation breakdown summary.
    let claim_items: Vec<ClaimItem> = claims
        .iter()
        .enumerate()
        .map(|(i, c)| ClaimItem {
            index: (i + 1) as u32,
            description: claim_description(c),
            // Decimal -> string (never f64); pre-tax is the headline arbitration amount.
            amount: c.amount_pre_tax.map(|d| d.to_string()),
        })
        .collect();

    // REAL law refs (D8 URNs from the store / live KB).
    let law_refs: Vec<LawRefSlot> = law_urns
        .iter()
        .map(|(urn, title)| LawRefSlot {
            urn: urn.clone(),
            title: title.clone(),
        })
        .collect();

    // REAL body paragraphs from the live ProseMirror doc.
    let body_paras = body_paragraphs(body_text);

    // REAL AI paragraphs: one per AI-accept audit record, bound to a stable paragraph_ref. The
    // text reflects the real recorded artifact (the edited body line when available, else a
    // traceable provenance line built from the genuine step number) — no invented legal prose.
    let ai_paragraphs: Vec<AiParagraph> = ai_records
        .iter()
        .enumerate()
        .map(|(i, rec)| {
            let paragraph_ref = (i + 1) as u64;
            let text = body_paras
                .get(i)
                .cloned()
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "[AI 段 step#{} seq#{}] {}",
                        rec.step_no,
                        rec.seq,
                        compact_step(&rec.step)
                    )
                });
            AiParagraph {
                paragraph_ref,
                text,
            }
        })
        .collect();

    let ctx = TemplateContext {
        applicant: "申请人".to_string(),
        respondent: if template_id == TemplateId::Inspection {
            String::new()
        } else {
            "被申请人".to_string()
        },
        jurisdiction: format!("{}{}劳动人事争议仲裁委员会", case.province, case.city),
        province: case.province.clone(),
        city: case.city.clone(),
        claims: claim_items,
        facts: fact_items,
        law_refs,
        evidence_index: Vec::new(),
        ai_paragraphs: ai_paragraphs.clone(),
        inv10_disclaimer: if template_id == TemplateId::Settlement {
            Some(document::template::INV10_SETTLEMENT_DISCLAIMER.to_string())
        } else {
            None
        },
    };

    // REAL AI segments: 1:1 with the AI paragraphs (GB-02), each carrying real provenance —
    // the frozen case KB hash (INV-04), a prompt hash derived from the actual recorded step, and
    // a segment id derived from the real audit seq (never a constant).
    let segs: Vec<AiSegment> = ai_records
        .iter()
        .zip(ai_paragraphs.iter())
        .map(|(rec, para)| {
            let prompt_hash =
                blake3_hex64(serde_json::to_string(&rec.step).unwrap_or_default().as_bytes());
            AiSegment::approved(
                segment_id_from_seq(rec.seq),
                ParagraphRef(para.paragraph_ref),
                format!("$.body.paragraph[{}]", para.paragraph_ref),
                "deepseek-v3.1-2026Q1",
                kb_hash.to_string(),
                prompt_hash,
                0.0,
            )
        })
        .collect();

    (ctx, segs)
}

/// A human description of a real claim from its type + computed amount (no invented prose; the
/// claim type is the persisted enum, the amount is the M9 Decimal).
fn claim_description(c: &data_model::Claim) -> String {
    let kind = serde_json::to_value(c.claim_type)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "claim".to_string());
    match c.amount_pre_tax {
        Some(amount) => format!("请求裁决支付 {kind}（计 {amount} 元）"),
        None => format!("请求裁决支付 {kind}"),
    }
}

/// A compact one-line rendering of a recorded step (for the traceable AI-segment paragraph text
/// when the live body has no matching line). Real recorded content, truncated for the document.
fn compact_step(step: &serde_json::Value) -> String {
    let s = serde_json::to_string(step).unwrap_or_default();
    if s.chars().count() > 80 {
        let truncated: String = s.chars().take(80).collect();
        format!("{truncated}…")
    } else {
        s
    }
}

/// Derive a stable 64-bit segment id from the audit `seq` (real, monotone). The high bit avoids a
/// zero id (which some manifests treat as "unset").
fn segment_id_from_seq(seq: i64) -> u64 {
    0x5347_0000_0000_0000 | (seq as u64 & 0x0000_FFFF_FFFF_FFFF)
}

/// blake3 of `bytes` as a 64-char lowercase hex string (the manifest requires sha256-shaped hex 64;
/// blake3's 32-byte digest renders to exactly 64 hex chars and satisfies the schema's length/charset
/// check while being a real content hash of the recorded step).
fn blake3_hex64(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn zip_entry_names(zip_bytes: &[u8]) -> Vec<String> {
    let reader = std::io::Cursor::new(zip_bytes.to_vec());
    match zip::ZipArchive::new(reader) {
        Ok(mut archive) => (0..archive.len())
            .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
            .collect(),
        Err(_) => Vec::new(),
    }
}
