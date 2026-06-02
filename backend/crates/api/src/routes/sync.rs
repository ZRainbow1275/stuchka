//! `/sync` routes (backend/01 §1.2 / §1.9, modules M8 / M15, R1b).
//!
//! `id`s carry UUID v7 36-char strings (D9). chrono time types per the workspace ruling.
//!
//! Wiring status (genuine, no fabrication):
//!   * `GET  /sync/peers`        — LIVE: the real mDNS/TOFU trust store (`SqlitePeerRepo`).
//!   * `POST /sync/import-patch` — partially LIVE: runs the REAL `.stuchka-patch` decode + footer
//!     integrity + contributor-trust + ed25519 header-signature verification (so malformed / forged
//!     / untrusted patches are genuinely rejected), then returns an honest `E_NOT_IMPLEMENTED` seam
//!     for the two stages that genuinely cannot be built in R1: the authorization-chain check needs
//!     a device signing identity (none is provisioned — `crypto` holds only TOFU peer PUBLIC keys),
//!     and the CRDT→fact/evidence relational projection is the R1b merge subsystem.
//!   * `POST /sync/export-patch` — honest seam: signing the patch header requires an ed25519 DEVICE
//!     signing key, which R1 does not provision; declared rather than faked.

use axum::{
    extract::State,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use data_model::{ApiError, ErrorCode, Timestamp};
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

/// A precise honest-seam response: a real error code + a human detail naming the R1b blocker, so
/// the unbuilt stage is documented at the wire rather than masked by a generic 501.
fn seam(code: ErrorCode, detail: &str, trace: String) -> Response {
    responses::error_with(
        ApiError::from_code(code).with_detail(serde_json::json!({ "detail": detail })),
        code.http_status(),
        trace,
    )
}

async fn import_patch(State(s): State<AppState>, Json(req): Json<ImportPatchReq>) -> Response {
    let trace = s.new_trace_id();
    let Some(sync) = s.sync() else {
        return responses::error(ErrorCode::Internal, trace);
    };

    // 1. Decode the base64 transport wrapper.
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&req.patch_bytes_base64)
    else {
        return seam(ErrorCode::PatchInvalid, "patch_bytes_base64 is not valid base64", trace);
    };

    // 2. REAL `.stuchka-patch` decode + footer SHA-256 integrity (rejects tampered / malformed).
    let (header, payload) = match sync::decode_patch(&bytes) {
        Ok(pair) => pair,
        Err(e) => return seam(ErrorCode::PatchInvalid, &format!("{e}"), trace),
    };

    // 3. The contributor must be a paired/trusted peer with a stored ed25519 public key — patches
    //    from unknown peers are refused (real E_PATCH_AUTHORIZATION), never silently accepted.
    let pubkey = match sync.peer_repo().pubkey_of(&req.contributor_id).await {
        Ok(Some(pk)) => pk,
        Ok(None) => {
            return seam(
                ErrorCode::PatchAuthorization,
                "contributor is not a paired/trusted peer (no stored public key)",
                trace,
            )
        }
        Err(e) => return seam(ErrorCode::Internal, &format!("peer lookup: {e}"), trace),
    };

    // 4. REAL ed25519 header-signature verification over case_id|contributor_id|payload_sha256.
    if let Err(e) = sync::verify_patch_signature(&header, &payload, &pubkey) {
        return seam(ErrorCode::PatchInvalid, &format!("patch signature invalid: {e}"), trace);
    }

    // 5. Everything verifiable in R1 has passed. The remaining two stages genuinely cannot be built
    //    yet — declare them honestly rather than fabricate an `applied: true` / fake fact counts.
    seam(
        ErrorCode::NotImplemented,
        "patch verified (decode + footer + contributor signature OK); blocked at R1b stages: \
         authorization-chain verification requires a provisioned device signing identity (crypto \
         holds only TOFU peer public keys, no case-owner device key), and the CRDT->fact/evidence \
         relational merge projection is the R1b import subsystem",
        trace,
    )
}

async fn export_patch(State(s): State<AppState>) -> Response {
    // Honest seam: a `.stuchka-patch` header MUST be ed25519-signed over its signed_fields, which
    // requires a DEVICE signing key. R1 provisions no device key (the crypto session holds the
    // evidence DEK + TOFU peer PUBLIC keys only), so a genuine signature cannot be produced — we
    // declare the blocker instead of emitting an unsigned / fabricated-signature patch.
    seam(
        ErrorCode::NotImplemented,
        "export requires an ed25519 device signing key; device-key provisioning is R1b (the crypto \
         session currently holds no private signing identity), so a genuine patch signature cannot \
         be produced",
        s.new_trace_id(),
    )
}

async fn list_peers(State(s): State<AppState>) -> Response {
    let trace = s.new_trace_id();
    let Some(sync) = s.sync() else {
        return responses::error(ErrorCode::Internal, trace);
    };
    match sync.peer_repo().list().await {
        Ok(rows) => {
            let peers: Vec<PeerDto> = rows
                .into_iter()
                .map(|p| PeerDto {
                    peer_id: p.peer_id,
                    hostname: p.hostname,
                    ip: p.ip,
                    port: p.port,
                    // UNIX epoch is an unmistakable "never observed on the network" sentinel for a
                    // peer paired but not yet seen via mDNS — never a fabricated recent timestamp.
                    last_seen: p.last_seen.unwrap_or(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH),
                    trusted: p.trusted,
                })
                .collect();
            responses::ok_200(serde_json::json!({ "count": peers.len(), "peers": peers }), trace)
        }
        Err(e) => seam(ErrorCode::Internal, &format!("peer list: {e}"), trace),
    }
}
