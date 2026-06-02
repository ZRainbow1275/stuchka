//! `/sync` routes (backend/01 §1.2 / §1.9, modules M8 / M15, R1b).
//!
//! `id`s carry UUID v7 36-char strings (D9). chrono time types per the workspace ruling.
//! Handlers return `501 E_NOT_IMPLEMENTED`.

use axum::{
    extract::State,
    response::Response,
    routing::{get, post},
    Router,
};
use data_model::Timestamp;
use serde::{Deserialize, Serialize};

use crate::routes::fact::AuthLink;
use crate::{responses, state::AppState};

/// `POST /sync/import-patch` request (backend/01 §1.9).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPatchReq {
    /// Yjs binary update, base64.
    pub patch_bytes_base64: String,
    pub contributor_id: String,
    pub authorization_chain: Vec<AuthLink>,
    pub target_case_id: String,
}

/// `POST /sync/import-patch` response (backend/01 §1.9).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPatchResp {
    pub applied: bool,
    pub conflicts: Vec<ConflictMarker>,
    pub new_facts: u32,
    pub new_evidences: u32,
}

/// A Yjs auto-merge conflict block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictMarker {
    pub conflict_id: String,
    pub path: String,
}

/// `GET /sync/peers` item (backend/01 §1.9).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerDto {
    /// ed25519 public key base58 (not a D9 object id).
    pub peer_id: String,
    pub hostname: String,
    /// LAN IP.
    pub ip: String,
    pub port: u16,
    pub last_seen: Timestamp,
    /// User has approved this peer.
    pub trusted: bool,
}

/// Register the `/sync` routes (R1b).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sync/import-patch", post(import_patch))
        .route("/sync/export-patch", post(export_patch))
        .route("/sync/peers", get(list_peers))
}

async fn import_patch(State(s): State<AppState>) -> Response {
    responses::not_implemented(s.new_trace_id())
}

async fn export_patch(State(s): State<AppState>) -> Response {
    responses::not_implemented(s.new_trace_id())
}

async fn list_peers(State(s): State<AppState>) -> Response {
    responses::not_implemented(s.new_trace_id())
}
