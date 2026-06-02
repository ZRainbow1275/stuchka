//! `/kb` routes (backend/01 §1.2, module M6).
//!
//! `GET /kb/version` (current KB hash + label + update time) and `POST /kb/refresh` (manual pull).
//! Both read the live `crates/kb` manifest / version held in `AppState`. R1a's refresh re-validates
//! the seeded manifest and reports the freshness level; the real network pull (GitHub Pages →
//! jsDelivr → mirror) is the R1b fetch path.

use axum::{
    extract::State,
    response::Response,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use data_model::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// `GET /kb/version` response (backend/01 §1.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbVersionDto {
    /// Current KB content hash (INV-04 freeze key).
    pub version_hash: String,
    pub version_label: String,
    pub updated_at: Timestamp,
}

/// `POST /kb/refresh` response (backend/01 §1.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbRefreshDto {
    pub version_hash: String,
    pub version_label: String,
    /// Whole-day age of the active KB.
    pub age_days: u32,
    /// `fresh` | `stale` | `expired`.
    pub freshness: String,
    /// Whether a network refresh actually changed the active version (R1a: false, no network).
    pub changed: bool,
}

/// Register `POST /kb/refresh` and `GET /kb/version`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/kb/refresh", post(kb_refresh))
        .route("/kb/version", get(kb_version))
}

async fn kb_version(State(s): State<AppState>) -> Response {
    let trace = s.new_trace_id();
    let (Some(manifest), Some(version)) = (s.kb_manifest(), s.kb_version()) else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    let dto = KbVersionDto {
        version_hash: manifest.global_hash.clone(),
        version_label: version.version_label.clone(),
        updated_at: manifest.generated_at,
    };
    responses::ok_200(dto, trace)
}

async fn kb_refresh(State(s): State<AppState>) -> Response {
    let trace = s.new_trace_id();
    let Some(manifest) = s.kb_manifest() else {
        return responses::error(data_model::ErrorCode::Internal, trace);
    };
    // R1a: re-validate the active manifest (KBC-02/03 invariants) instead of hitting the network.
    if manifest.validate().is_err() {
        return responses::error(data_model::ErrorCode::Internal, trace);
    }
    let now = Utc::now();
    let freshness = match kb::freshness(manifest.generated_at, now) {
        kb::FreshnessLevel::Fresh => "fresh",
        kb::FreshnessLevel::Stale => "stale",
        kb::FreshnessLevel::Expired => "expired",
    };
    let dto = KbRefreshDto {
        version_hash: manifest.global_hash.clone(),
        version_label: s
            .kb_version()
            .map(|v| v.version_label.clone())
            .unwrap_or_default(),
        age_days: kb::age_days(manifest.generated_at, now),
        freshness: freshness.to_string(),
        changed: false,
    };
    responses::ok_200(dto, trace)
}
