//! `/health` — process health probe (backend/01 §1.2; rust→flutter liveness, R1).
//!
//! Fully live: returns `200 { data: HealthDto, error: null, traceId }`. Used by the Flutter
//! parent to confirm the child is serving after the READY handshake.

use axum::{extract::State, response::Response, routing::get, Router};
use serde::{Deserialize, Serialize};

use crate::{responses, state::AppState};

/// Health payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDto {
    /// Always `"ok"` when the process is serving.
    pub status: String,
    /// Crate version (`CARGO_PKG_VERSION`).
    pub version: String,
}

/// Register `GET /health`.
pub fn routes() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

async fn health(State(state): State<AppState>) -> Response {
    let dto = HealthDto {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    responses::ok_200(dto, state.new_trace_id())
}
