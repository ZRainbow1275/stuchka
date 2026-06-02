//! `/sync` route integration tests (backend/01 §1.9, M8/M15 R1b).
//!
//! Boots the fully-wired in-memory app (real `SqlitePeerRepo` over the main pool + the live sync
//! provider) on a loopback port and drives the `/sync` endpoints over HTTP + Bearer, asserting the
//! GENUINE behaviour: `GET /sync/peers` returns the real (empty) trust store; `POST
//! /sync/import-patch` runs the real decode + trust + signature verification (rejecting malformed
//! and untrusted patches with the correct §1.12 codes); and both blocked stages surface as honest
//! seams (`E_NOT_IMPLEMENTED`) rather than fabricated success.

use std::time::Duration;

use api::{router, AppState};
use base64::Engine;
use serde_json::{json, Value};
use stuchka_core::BootServices;
use sync::{encode_patch, CaseDoc, PatchHeader, PatchScope};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn spawn_app() -> String {
    let blob_dir = std::env::temp_dir().join(format!("stuchka-sync-{}", uuid::Uuid::now_v7()));
    let services = BootServices::initialize_in_memory(blob_dir)
        .await
        .expect("init in-memory services");
    let secrets = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("config")
        .join("secrets.toml");
    let state = AppState::from_boot_wired(TOKEN, services, secrets)
        .await
        .expect("wire app state");
    let app = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://127.0.0.1:{port}")
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .no_proxy()
        .build()
        .expect("build client")
}

/// A structurally-valid `.stuchka-patch` (real codec) so the import path reaches the trust check.
fn valid_patch_base64() -> String {
    let case_id = "0190b2aa-0000-7000-8000-000000000001";
    let doc = CaseDoc::new(case_id);
    let header = PatchHeader {
        case_id: case_id.to_string(),
        contributor_id: "unpaired-contributor-peer".to_string(),
        authorization_chain: vec![],
        from_kb_version: "kbhash".to_string(),
        scope: PatchScope::Full,
        issued_at: "2026-01-02T00:00:00Z".to_string(),
        signature: "c2ln".to_string(), // base64 "sig"; never reached for an unpaired contributor
        signed_fields: "case_id|contributor_id|payload_sha256".to_string(),
    };
    let bytes = encode_patch(&doc, PatchScope::Full, header).expect("encode patch");
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[test]
fn sync_peers_is_live_and_empty_on_fresh_boot() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();
    let body: Value = c
        .get(format!("{base}/sync/peers"))
        .bearer_auth(TOKEN)
        .send()
        .expect("GET /sync/peers")
        .json()
        .expect("json");
    assert!(body["error"].is_null(), "peers must succeed: {body}");
    assert_eq!(body["data"]["count"], 0, "fresh boot has no peers");
    assert!(body["data"]["peers"].as_array().unwrap().is_empty());
}

#[test]
fn import_patch_rejects_malformed_bytes() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();
    // Valid base64 of non-patch bytes -> the real decoder rejects (bad magic) with E_PATCH_INVALID.
    let garbage = base64::engine::general_purpose::STANDARD.encode(b"this is not a patch");
    let body: Value = c
        .post(format!("{base}/sync/import-patch"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "patchBytesBase64": garbage,
            "contributorId": "peer-x",
            "authorizationChain": [],
            "targetCaseId": "0190b2aa-0000-7000-8000-000000000001"
        }))
        .send()
        .expect("POST /sync/import-patch")
        .json()
        .expect("json");
    assert_eq!(body["error"]["code"], "E_PATCH_INVALID", "malformed patch: {body}");
}

#[test]
fn import_patch_refuses_unpaired_contributor() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();
    // A structurally-valid patch, but the contributor is not a paired/trusted peer -> the real
    // trust check refuses it (E_PATCH_AUTHORIZATION), never silently accepting an unknown sender.
    let body: Value = c
        .post(format!("{base}/sync/import-patch"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "patchBytesBase64": valid_patch_base64(),
            "contributorId": "unpaired-contributor-peer",
            "authorizationChain": [],
            "targetCaseId": "0190b2aa-0000-7000-8000-000000000001"
        }))
        .send()
        .expect("POST /sync/import-patch")
        .json()
        .expect("json");
    assert_eq!(
        body["error"]["code"], "E_PATCH_AUTHORIZATION",
        "unpaired contributor must be refused: {body}"
    );
}

#[test]
fn export_patch_is_an_honest_device_key_seam() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();
    let resp = c
        .post(format!("{base}/sync/export-patch"))
        .bearer_auth(TOKEN)
        .json(&json!({}))
        .send()
        .expect("POST /sync/export-patch");
    assert_eq!(resp.status().as_u16(), 501, "export is an honest 501 seam");
    let body: Value = resp.json().expect("json");
    assert_eq!(body["error"]["code"], "E_NOT_IMPLEMENTED");
    let detail = body["error"]["detail"]["detail"].as_str().unwrap_or("");
    assert!(detail.contains("device"), "seam names the device-key blocker: {body}");
}
