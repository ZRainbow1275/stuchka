//! `/diagnose` routes (prd/04 §4.1, module M1) — the DETERMINISTIC diagnosis engine surface.
//!
//! Drives the REAL [`rule_engine::DiagnosisEngine`] (问诊树 + 85-subcategory catalog, INV-01 pure
//! rules, AI 不参与). Three endpoints:
//! - `GET  /diagnose/tree`  — the root 问诊树 question (20 大类分流), to start the interactive walk.
//! - `POST /diagnose/step`  — advance one question: `(nodeId, answerValue) -> next question | done`.
//! - `POST /diagnose`       — run the full deterministic diagnosis over a completed answer path,
//!   returning the §4.1.2 structured [`DiagnosisOutput`]; when `caseId` is supplied the diagnosed
//!   subcategory is persisted onto the case (replacing the `LD-00-00` placeholder) and the result
//!   is audited (`AiDiagnosisOutput`, INV-06). No stubs — every path truly runs the engine.

use axum::{
    extract::State,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use data_model::{DisputeSubtype, IdentityType};
use rule_engine::diagnosis::tree::Step;
use rule_engine::{DiagnosisInput, DiagnosisOutput};
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// `POST /diagnose` request (prd §4.1.1). The free-text description is mapped to the structured
/// `answer_path` upstream (LLM assist, INV-01); the engine itself consumes only the structured
/// answers. `caseId` (optional) persists the diagnosed subcategory onto an existing case.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnoseReq {
    pub identity_type: IdentityType,
    pub dispute_subtype: DisputeSubtype,
    /// Ordered 问诊树 answer values (one per question).
    pub answer_path: Vec<String>,
    /// Questions the user skipped (≥ 3 → 启发式追问 / abstention, prd §4.1.4).
    #[serde(default)]
    pub skipped_questions: u32,
    /// Optional case to persist the diagnosed subcategory onto.
    #[serde(default)]
    pub case_id: Option<String>,
}

/// One question rendered for the UI (prd §4.1.4: 每问只问一项).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionDto {
    pub node_id: String,
    pub prompt_zh: String,
    pub answers: Vec<AnswerDto>,
}

/// One answer option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerDto {
    pub value: String,
    pub label_zh: String,
}

/// `POST /diagnose/step` request.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepReq {
    pub node_id: String,
    pub answer_value: String,
}

/// `POST /diagnose/step` response: either the next question or a terminal subcategory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "step")]
pub enum StepResp {
    #[serde(rename = "ask")]
    Ask { question: QuestionDto },
    #[serde(rename = "done")]
    Done { subcategory: String },
}

/// Register the `/diagnose` routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/diagnose", post(diagnose))
        .route("/diagnose/tree", get(diagnose_tree))
        .route("/diagnose/step", post(diagnose_step))
}

/// `GET /diagnose/tree` — the root 问诊树 question.
async fn diagnose_tree(State(s): State<AppState>) -> Response {
    let trace = s.new_trace_id();
    let Some(engine) = s.diagnosis_engine() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let (node_id, q) = match engine.tree().root_question() {
        Ok(v) => v,
        Err(_) => return responses::error(data_model::ErrorCode::Internal, trace),
    };
    let dto = QuestionDto {
        node_id: node_id.to_string(),
        prompt_zh: q.prompt_zh.clone(),
        answers: q
            .answers
            .iter()
            .map(|a| AnswerDto {
                value: a.value.clone(),
                label_zh: a.label_zh.clone(),
            })
            .collect(),
    };
    responses::ok_200(dto, trace)
}

/// `POST /diagnose/step` — advance one question in the interactive walk.
async fn diagnose_step(State(s): State<AppState>, Json(req): Json<StepReq>) -> Response {
    let trace = s.new_trace_id();
    let Some(engine) = s.diagnosis_engine() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    match engine.tree().step(&req.node_id, &req.answer_value) {
        Some(Step::Ask { node_id, question }) => responses::ok_200(
            StepResp::Ask {
                question: QuestionDto {
                    node_id,
                    prompt_zh: question.prompt_zh,
                    answers: question
                        .answers
                        .into_iter()
                        .map(|a| AnswerDto {
                            value: a.value,
                            label_zh: a.label_zh,
                        })
                        .collect(),
                },
            },
            trace,
        ),
        Some(Step::Done { subcategory }) => {
            responses::ok_200(StepResp::Done { subcategory }, trace)
        }
        // Unknown node / invalid answer → bad request (never a guess, C-C-6).
        None => responses::error(data_model::ErrorCode::BadRequest, trace),
    }
}

/// `POST /diagnose` — run the full deterministic diagnosis.
async fn diagnose(State(s): State<AppState>, Json(req): Json<DiagnoseReq>) -> Response {
    let trace = s.new_trace_id();
    let Some(engine) = s.diagnosis_engine() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };

    let input = DiagnosisInput {
        identity_type: req.identity_type,
        dispute_subtype: req.dispute_subtype,
        answer_path: req.answer_path.clone(),
        skipped_questions: req.skipped_questions,
    };
    let output = engine.diagnose(&input);

    match &output {
        DiagnosisOutput::Ok(result) => {
            // When a case id is supplied, persist the diagnosed subcategory + coverage tier onto
            // the case (replacing the LD-00-00 placeholder) and move it draft -> diagnosed.
            if let Some(case_id_str) = &req.case_id {
                let Ok(case_id) = uuid::Uuid::parse_str(case_id_str) else {
                    return responses::error(data_model::ErrorCode::BadRequest, trace);
                };
                let Some(store) = s.store() else {
                    return responses::error(data_model::ErrorCode::Internal, trace);
                };
                let mut case = match store.case.get(&case_id).await {
                    Ok(Some(c)) => c,
                    Ok(None) => return responses::error(data_model::ErrorCode::NotFound, trace),
                    Err(e) => return responses::from_db_error(&e, trace),
                };
                // §4.1.3: a frozen case is terminal — modification = re-creating the case. Refuse to
                // re-diagnose onto a frozen case (INV-04 / state-machine terminal).
                if case.status == data_model::CaseStatus::Frozen {
                    return responses::error(data_model::ErrorCode::InvalidTransition, trace);
                }
                case.dispute_category = result.dispute_category.clone();
                case.coverage_tier = result.coverage_tier;
                // Advance draft -> diagnosed when the state machine allows it (idempotent if already
                // diagnosed: only flip from Draft).
                if case.status == data_model::CaseStatus::Draft {
                    case.status = data_model::CaseStatus::Diagnosed;
                }
                case.updated_at = Utc::now();
                if let Err(e) = store.case.update(&case).await {
                    return responses::from_db_error(&e, trace);
                }

                // Audit the diagnosis output (INV-06 mandatory AI key-suggestion record).
                if let Err(resp) = crate::audit_helper::append_state_change(
                    &s,
                    audit::AuditReason::AiDiagnosisOutput,
                    case_id,
                    serde_json::json!({
                        "case_id": case_id.to_string(),
                        "dispute_category": result.dispute_category,
                        "coverage_tier": result.coverage_tier,
                        "coverage_tag": result.coverage_tag,
                        "confidence": result.confidence,
                        "engine": "deterministic_decision_tree",
                    }),
                    &trace,
                )
                .await
                {
                    return resp;
                }
            }
            responses::ok_200(&output, trace)
        }
        // Abstention (prd §4.1.5): return the structured `out_of_scope` object as a 200 so the UI
        // renders its reasons / next_actions verbatim — never a fabricated subcategory. The
        // `status: "out_of_scope"` tag is the discriminator (mirrors the rule-engine RuleOutcome /
        // the dispatcher Low-confidence abstention contract, INV-01 / C-C-7). The DB is not
        // mutated (no determinate subcategory to persist) even when a caseId is supplied.
        DiagnosisOutput::OutOfScope { .. } => responses::ok_200(&output, trace),
    }
}
