//! `/law-ref` routes (backend/01 §1.2 / §1.7, module M6).
//!
//! `id` is a D8 URN string (URL-encoded path segment), not a UUID (§1.7.1). `GET /law-ref` runs
//! the real `crates/kb` BM25 hybrid search (with optional province/city filter); `GET /law-ref/:id`
//! resolves a single URN against the seeded corpus and validates it via the data-model D8 parser.

use axum::{
    extract::{Path, Query, State},
    response::Response,
    routing::get,
    Router,
};
use data_model::{LawLevel, PlainDate};
use kb::{LawSearch, SearchQuery};
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// Law-reference DTO (backend/01 §1.7.1). `id` = D8 URN.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LawRefDto {
    pub id: String,
    pub title: String,
    pub level: LawLevel,
    pub issued_by: String,
    pub effective_from: PlainDate,
    pub effective_to: Option<PlainDate>,
    pub article_no: Option<String>,
    pub body_md: String,
    /// Normalized-text sha256 (INV-04 freeze boundary, D8).
    pub content_hash: String,
    pub kb_version: String,
}

/// A search hit projection (backend/01 §1.7.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LawRefHitDto {
    pub stable_id: String,
    pub title: String,
    pub score: f32,
    pub matched_spans: Vec<(usize, usize)>,
}

/// `GET /law-ref` query (backend/01 §1.7.2).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LawRefQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub province: Option<String>,
    pub city: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Register `GET /law-ref` and `GET /law-ref/:id`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/law-ref", get(search_law_ref))
        .route("/law-ref/:id", get(get_law_ref))
}

async fn search_law_ref(State(s): State<AppState>, Query(query): Query<LawRefQuery>) -> Response {
    let trace = s.new_trace_id();
    let Some(index) = s.kb_index() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let kb_query = SearchQuery {
        q: query.q.unwrap_or_default(),
        category: query.category,
        province: query.province,
        city: query.city,
        limit: query.limit.unwrap_or(20),
        offset: query.offset.unwrap_or(0),
    };
    match index.search(&kb_query) {
        Ok(hits) => {
            let dtos: Vec<LawRefHitDto> = hits
                .into_iter()
                .map(|h| LawRefHitDto {
                    title: sample_title(&h.stable_id).unwrap_or_default(),
                    stable_id: h.stable_id,
                    score: h.score,
                    matched_spans: h.matched_spans,
                })
                .collect();
            responses::ok_200(dtos, trace)
        }
        Err(e) => {
            tracing::error!(error = %e, "kb search failed");
            responses::error(data_model::ErrorCode::Internal, trace)
        }
    }
}

async fn get_law_ref(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    let trace = s.new_trace_id();
    // The path segment is URL-encoded; axum decodes percent-escapes into `id`.
    // Validate the D8 URN grammar via the single data-model parser.
    let parts = match data_model::parse_law_ref(&id) {
        Ok(p) => p,
        Err(_) => return responses::error(data_model::ErrorCode::BadRequest, trace),
    };

    // Resolve against the seeded corpus (the dev KB source of truth).
    let Some(sample) = kb::sample_data::SAMPLE_LAWS
        .iter()
        .find(|l| l.stable_id == id)
    else {
        return responses::error(data_model::ErrorCode::NotFound, trace);
    };

    let kb_version = s
        .kb_version()
        .map(|v| v.version_label.clone())
        .unwrap_or_default();

    let dto = LawRefDto {
        id: id.clone(),
        title: sample.title.to_string(),
        level: level_for(&parts.title),
        issued_by: String::new(),
        effective_from: parts.version_date,
        effective_to: None,
        article_no: Some(format!("§{}", parts.article)),
        body_md: sample.body.to_string(),
        content_hash: data_model::content_hash(sample.body),
        kb_version,
    };
    responses::ok_200(dto, trace)
}

/// Best-effort title for a seeded URN (used in search-hit projection).
fn sample_title(stable_id: &str) -> Option<String> {
    kb::sample_data::SAMPLE_LAWS
        .iter()
        .find(|l| l.stable_id == stable_id)
        .map(|l| l.title.to_string())
}

/// Map a statute name onto its hierarchy level (best-effort heuristic for the seeded corpus).
fn level_for(law_name: &str) -> LawLevel {
    if law_name.contains("解释") {
        LawLevel::JudicialInterpretation
    } else if law_name.contains("条例") {
        LawLevel::AdministrativeRegulation
    } else if law_name.contains("规定") || law_name.contains("办法") {
        LawLevel::DepartmentalRule
    } else if law_name.starts_with("中华人民共和国") {
        LawLevel::Statute
    } else {
        // Province-scoped statutes (e.g. 广东省工资支付条例) are local regulations.
        LawLevel::LocalRegulation
    }
}
