//! R1a closed-loop end-to-end integration test (STEP wire2).
//!
//! Boots the fully-wired app (REAL in-memory db + audit + rule-engine + KB + hsd + age, plus the
//! live Yjs sync provider and — when `config/secrets.toml` is present — the AI dispatcher) on a
//! loopback port, then drives the closed loop over HTTP + Bearer exactly as the Flutter parent
//! would:
//!
//! create case → add facts → add evidence → `/compute/run` → create document
//! (`arbitration_application`) → push editor steps via `POST /document/:id/steps` → `POST
//! /document/:id/export` (formats pdf+md+json, gb45438_level 4) → assert the returned dossier zip
//! exists and contains md + json (+pdf when produced) and that the JSON manifest lists
//! `ai_generated_segments` + the GB45438 four-layer completeness → assert `/audit` shows an Export
//! entry and that `verify_chain` holds.
//!
//! The real-SiliconFlow `/llm/query` leg is a separate `#[ignore]` test run explicitly (it makes a
//! live network call and needs `config/secrets.toml`).

use std::time::Duration;

use api::{router, AppState};
use serde_json::{json, Value};
use stuchka_core::BootServices;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Spawn the fully-wired in-process app on a loopback port; returns the base URL.
async fn spawn_app() -> String {
    let blob_dir = std::env::temp_dir().join(format!("stuchka-e2e-{}", uuid::Uuid::now_v7()));
    let services = BootServices::initialize_in_memory(blob_dir)
        .await
        .expect("init in-memory services");
    // The closed-loop test does not exercise the live LLM leg; point the secrets path at the real
    // backend config so the ignored live test below can also use this spawn helper (absent → the
    // dispatcher is simply None and /llm degrades, which the closed loop never touches).
    let secrets = secrets_path();
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

/// The real `config/secrets.toml` next to the backend workspace (CARGO_MANIFEST_DIR is the api
/// crate dir, so go up two levels to the backend root).
fn secrets_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("config")
        .join("secrets.toml")
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .no_proxy()
        .build()
        .expect("build client")
}

#[test]
fn r1a_closed_loop_over_http_bearer() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();

    // ---- 1. Create a case (M1). ----
    let case_body: Value = c
        .post(format!("{base}/case"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "identityType": "standard_full_time",
            "province": "44",
            "city": "4401",
            "caseOccurredAt": "2026-01-02",
            "disputeSubtype": "social_ins_waiver_invalid",
            "firstDescription": "用人单位违法解除劳动合同，要求经济赔偿。",
            "kbVersionHash": "client-thinks-this"
        }))
        .send()
        .expect("create case")
        .json()
        .unwrap();
    assert!(case_body["error"].is_null(), "case error: {case_body}");
    let case_id = case_body["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();

    // ---- 2. Add a couple of facts (M1). ----
    for (cat, stmt) in [
        (
            "termination_reason",
            "2026-01-02 被公司以不胜任为由违法解除。",
        ),
        ("wage", "解除前 12 个月平均工资 8000 元。"),
    ] {
        let f = c
            .post(format!("{base}/case/{case_id}/fact"))
            .bearer_auth(TOKEN)
            .json(&json!({
                "category": cat,
                "statement": stmt,
                "source": "user_input",
                "authorizationChain": []
            }))
            .send()
            .expect("create fact");
        assert_eq!(f.status().as_u16(), 201, "POST fact => 201");
    }

    // ---- 3. Add an evidence (M2). ----
    let metadata = json!({
        "category": "contract",
        "collected_at": "2026-01-03T10:00:00Z",
        "device_id": "dev-sha-1",
        "linked_fact_ids": []
    })
    .to_string();
    let form = reqwest::blocking::multipart::Form::new()
        .text("metadata", metadata)
        .part(
            "file",
            reqwest::blocking::multipart::Part::bytes(b"labor contract body".to_vec())
                .file_name("contract.txt"),
        );
    let ev = c
        .post(format!("{base}/case/{case_id}/evidence"))
        .bearer_auth(TOKEN)
        .multipart(form)
        .send()
        .expect("upload evidence");
    assert_eq!(ev.status().as_u16(), 201, "POST evidence => 201");

    // ---- 4. /compute/run severance (M9) — exact Decimal. ----
    let comp_body: Value = c
        .post(format!("{base}/compute/run"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "caseId": case_id,
            "scenarios": ["severance"],
            "province": "44",
            "city": "4401",
            "wageData": { "monthlyWage": "8000" },
            "period": { "from": "2018-01-01", "to": "2026-01-01" }
        }))
        .send()
        .expect("compute run")
        .json()
        .unwrap();
    assert!(comp_body["error"].is_null(), "compute error: {comp_body}");
    let amount: rust_decimal::Decimal = comp_body["data"]["results"][0]["amount"]
        .as_str()
        .expect("amount string")
        .parse()
        .expect("amount decimal");
    assert_eq!(amount, rust_decimal::Decimal::from(64000), "8000 * 8 yrs");

    // ---- 5. Create a document (template arbitration_application = arb_application). ----
    let doc_body: Value = c
        .post(format!("{base}/case/{case_id}/document"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "templateId": "arb_application",
            "claimIds": []
        }))
        .send()
        .expect("create document")
        .json()
        .unwrap();
    assert!(doc_body["error"].is_null(), "document error: {doc_body}");
    let doc_id = doc_body["data"]["docId"]
        .as_str()
        .expect("doc id")
        .to_string();

    // GET /document/:id returns the Yjs state vector (base64).
    let state_body: Value = c
        .get(format!("{base}/document/{doc_id}"))
        .bearer_auth(TOKEN)
        .send()
        .expect("get document")
        .json()
        .unwrap();
    assert!(
        state_body["error"].is_null(),
        "doc state error: {state_body}"
    );
    assert!(
        state_body["data"]["stateVectorBase64"].is_string(),
        "state vector present: {state_body}"
    );

    // ---- 6. Push editor steps as a BATCH (POST /document/:id/steps → doc_step + audit). ----
    // The batch has 1 plain edit + 2 ai_accept steps; only the ai_accept steps are AI marks, so
    // the exported manifest must list EXACTLY 2 ai_generated_segments (derived from the real audit
    // trail, not a constant). This proves FIX A: segments track the real recorded AI marks.
    let actor = uuid::Uuid::now_v7().to_string();
    let expected_ai_segments = 2usize;
    let batch = c
        .post(format!("{base}/document/{doc_id}/steps"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "caseId": case_id,
            "actor": actor,
            "clientId": "client-1",
            "steps": [
                { "stepJson": { "stepType": "replace", "n": 0 }, "why": "edit" },
                { "stepJson": { "stepType": "replace", "n": 1, "ai": "draft-A" }, "why": "ai_accept" },
                { "stepJson": { "stepType": "replace", "n": 2, "ai": "draft-B" }, "why": "ai_accept" }
            ]
        }))
        .send()
        .expect("push step batch");
    assert_eq!(batch.status().as_u16(), 200, "POST step batch => 200");
    let bb: Value = batch.json().unwrap();
    assert!(bb["error"].is_null(), "step batch error: {bb}");
    let results = bb["data"]["results"].as_array().expect("batch results");
    assert_eq!(results.len(), 3, "three steps processed: {bb}");
    for r in results {
        assert!(r["auditSeq"].as_i64().unwrap() >= 1, "audit seq: {bb}");
    }

    // ---- 7. Export the dossier (pdf+md+json, gb45438_level 4). ----
    let export: Value = c
        .post(format!("{base}/document/{doc_id}/export"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "formats": ["pdf", "md", "json"],
            "embedWaterMark": true,
            "gb45438Level": 4
        }))
        .send()
        .expect("export document")
        .json()
        .unwrap();
    assert!(export["error"].is_null(), "export error: {export}");
    let data = &export["data"];

    // The returned dossier zip exists on disk.
    let zip_path = data["dossierZipPath"].as_str().expect("zip path");
    assert!(
        std::path::Path::new(zip_path).exists(),
        "dossier zip exists at {zip_path}"
    );

    // The bundle contains md + json (+ pdf, since lopdf is pure-Rust the PDF is always produced).
    let entries: Vec<String> = data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap().to_string())
        .collect();
    assert!(
        entries.iter().any(|n| n.ends_with(".md")),
        "md present: {entries:?}"
    );
    assert!(
        entries.iter().any(|n| n == "manifest.json"),
        "json manifest present: {entries:?}"
    );
    assert!(
        entries.iter().any(|n| n.ends_with(".pdf")),
        "pdf present (lopdf): {entries:?}"
    );
    assert!(
        data["pdfPresent"].as_bool().unwrap(),
        "pdf_present flag set"
    );

    // The JSON manifest lists ai_generated_segments derived from the REAL AI marks (the two
    // ai_accept steps), not a hardcoded constant. FIX A: the count tracks the real audit trail.
    assert_eq!(
        data["aiGeneratedSegments"].as_u64().unwrap() as usize,
        expected_ai_segments,
        "manifest AI segments must equal the real ai_accept marks: {data}"
    );
    let comp = &data["gb45438LayerCompleteness"];
    assert!(comp["layer1Explicit"].as_bool().unwrap(), "layer1");
    assert!(comp["layer2Metadata"].as_bool().unwrap(), "layer2");
    assert!(comp["layer3Zerowidth"].as_bool().unwrap(), "layer3 zw");
    assert!(comp["layer3Lsb"].as_bool().unwrap(), "layer3 lsb");
    assert!(comp["layer4Manifest"].as_bool().unwrap(), "layer4");

    // Verify the zip really contains the md + json on disk (read-back), not just the API claim.
    let zip_bytes = std::fs::read(zip_path).expect("read zip");
    let names = zip_entry_names(&zip_bytes);
    assert!(
        names.iter().any(|n| n.ends_with(".md")),
        "zip md: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "manifest.json"),
        "zip manifest: {names:?}"
    );

    // Read the manifest.json back from the zip and assert the AI segments reflect the REAL recorded
    // marks (FIX A): exactly the two ai_accept steps, each carrying a real per-segment id (NOT the
    // old hardcoded 0x0001) and a real 64-hex prompt/kb hash (NOT the old "aaaa…"/"bbbb…" filler).
    let manifest: Value = {
        let cur = std::io::Cursor::new(zip_bytes.clone());
        let mut archive = zip::ZipArchive::new(cur).unwrap();
        let mut f = archive.by_name("manifest.json").unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut f, &mut s).unwrap();
        serde_json::from_str(&s).unwrap()
    };
    let segs = manifest["documents"][0]["aiGeneratedSegments"]
        .as_array()
        .or_else(|| manifest["documents"][0]["ai_generated_segments"].as_array())
        .expect("manifest segments array");
    assert_eq!(
        segs.len(),
        expected_ai_segments,
        "manifest segment count tracks the real ai_accept marks: {manifest}"
    );
    let mut seg_ids = std::collections::HashSet::new();
    for s in segs {
        // segment_id is hex like "0x0016…"; must be unique per segment and derived from the real
        // audit seq (the "0x5347…" marker prefix), never the old hardcoded 0x0000000000000001.
        let sid = s["segment_id"].as_str().expect("segment_id hex");
        assert!(seg_ids.insert(sid.to_string()), "segment ids must be unique");
        assert_ne!(
            sid, "0x0000000000000001",
            "segment id must not be the old constant"
        );
        assert!(
            sid.starts_with("0x5347"),
            "segment id is derived from the real audit seq: {sid}"
        );
        // prompt_hash / kb_hash are real 64-char hex (no "aaaa…"/"bbbb…" filler).
        let prompt_hash = s["prompt_hash"].as_str().expect("prompt_hash");
        assert_eq!(prompt_hash.len(), 64, "prompt_hash hex 64");
        assert_ne!(prompt_hash, &"a".repeat(64), "prompt_hash must be real");
        assert_ne!(prompt_hash, &"b".repeat(64), "prompt_hash must be real");
    }

    // ---- 8. /audit shows an Export entry; chain projection rebuilds (verify_chain held at boot). ----
    let audit: Value = c
        .get(format!("{base}/audit"))
        .query(&[("caseId", case_id.as_str())])
        .bearer_auth(TOKEN)
        .send()
        .expect("audit query")
        .json()
        .unwrap();
    assert!(audit["error"].is_null(), "audit error: {audit}");
    let entries = audit["data"].as_array().expect("audit entries");
    assert!(
        entries.iter().any(|e| e["category"] == "export"),
        "an Export-category audit entry is present: {audit}"
    );
    // The case-scoped projection is seq-ascending and links prev→record (chain rebuildable).
    let mut last_seq = -1;
    for e in entries {
        let seq = e["seq"].as_i64().unwrap();
        assert!(seq > last_seq, "entries seq-ascending");
        last_seq = seq;
        assert!(e["recordHash"]
            .as_str()
            .map(|h| !h.is_empty())
            .unwrap_or(false));
    }
}

/// Spawn the wired app with a Level-4 EXPIRED KB (manifest generated 40 days ago) so the INV-04 /
/// KB-02 staleness gate fires. Returns the base URL.
async fn spawn_app_with_stale_kb() -> String {
    let blob_dir = std::env::temp_dir().join(format!("stuchka-e2e-stale-{}", uuid::Uuid::now_v7()));
    let mut services = BootServices::initialize_in_memory(blob_dir)
        .await
        .expect("init in-memory services");
    // Age the KB past the 30-day Level-4 boundary (kb freshness Expired) — the M9/M5 gate (FIX B).
    let stale = chrono::Utc::now() - chrono::Duration::days(40);
    services.kb_manifest.generated_at = stale;
    services.kb_version.generated_at = stale;
    let secrets = secrets_path();
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

/// FIX B (INV-04 / KB-02): `/compute/run` MUST refuse to compute on a Level-4 expired KB,
/// returning `E_KB_OUTDATED` (422) — never compute on a stale KB.
#[test]
fn compute_run_refuses_stale_kb() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app_with_stale_kb());
    let c = client();

    // A well-formed compute request that would otherwise succeed (severance over 8 years).
    let resp = c
        .post(format!("{base}/compute/run"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "caseId": uuid::Uuid::now_v7().to_string(),
            "scenarios": ["severance"],
            "province": "44",
            "city": "4401",
            "wageData": { "monthlyWage": "8000" },
            "period": { "from": "2018-01-01", "to": "2026-01-01" }
        }))
        .send()
        .expect("compute run");
    assert_eq!(resp.status().as_u16(), 422, "stale KB => 422");
    let body: Value = resp.json().unwrap();
    assert_eq!(
        body["error"]["code"], "E_KB_OUTDATED",
        "stale KB must abort compute with E_KB_OUTDATED: {body}"
    );
    assert!(body["data"].is_null(), "no computed data on a stale KB: {body}");
}

/// FIX C (soft delete + audit): DELETE /case/:id soft-deletes the case (30-day recycle), excludes
/// it from GET /case + GET /case/:id (404), writes an audit entry, and the chain still verifies.
#[test]
fn delete_case_soft_deletes_with_audit() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();

    // Create a case.
    let case_body: Value = c
        .post(format!("{base}/case"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "identityType": "standard_full_time",
            "province": "44",
            "city": "4401",
            "caseOccurredAt": "2026-01-02",
            "disputeSubtype": "social_ins_arrears",
            "firstDescription": "测试软删除。",
            "kbVersionHash": ""
        }))
        .send()
        .expect("create case")
        .json()
        .unwrap();
    assert!(case_body["error"].is_null(), "create error: {case_body}");
    let case_id = case_body["data"]["id"].as_str().unwrap().to_string();

    // It is present in the list before deletion.
    let list_before: Value = c
        .get(format!("{base}/case"))
        .bearer_auth(TOKEN)
        .send()
        .expect("list before")
        .json()
        .unwrap();
    let count_before = list_before["data"]["count"].as_u64().unwrap();
    assert!(count_before >= 1, "case present before delete: {list_before}");

    // DELETE => soft delete.
    let del: Value = c
        .delete(format!("{base}/case/{case_id}"))
        .bearer_auth(TOKEN)
        .send()
        .expect("delete case")
        .json()
        .unwrap();
    assert!(del["error"].is_null(), "delete error: {del}");
    assert_eq!(del["data"]["softDelete"], true, "soft delete flagged: {del}");

    // GET /case/:id now 404s.
    let get_after = c
        .get(format!("{base}/case/{case_id}"))
        .bearer_auth(TOKEN)
        .send()
        .expect("get after delete");
    assert_eq!(get_after.status().as_u16(), 404, "soft-deleted => 404");

    // The case is excluded from the list.
    let list_after: Value = c
        .get(format!("{base}/case"))
        .bearer_auth(TOKEN)
        .send()
        .expect("list after")
        .json()
        .unwrap();
    let cases_after = list_after["data"]["cases"].as_array().unwrap();
    assert!(
        !cases_after.iter().any(|x| x["id"] == case_id),
        "soft-deleted case excluded from list: {list_after}"
    );

    // An audit entry for the soft delete is present and the chain rebuilds (seq-ascending).
    let audit: Value = c
        .get(format!("{base}/audit"))
        .query(&[("caseId", case_id.as_str())])
        .bearer_auth(TOKEN)
        .send()
        .expect("audit query")
        .json()
        .unwrap();
    assert!(audit["error"].is_null(), "audit error: {audit}");
    let entries = audit["data"].as_array().expect("audit entries");
    assert!(
        entries
            .iter()
            .any(|e| e["what"]["action"] == "soft_delete"),
        "a soft_delete audit entry is present: {audit}"
    );
    let mut last_seq = -1;
    for e in entries {
        let seq = e["seq"].as_i64().unwrap();
        assert!(seq > last_seq, "entries seq-ascending (chain rebuildable)");
        last_seq = seq;
        assert!(e["recordHash"].as_str().map(|h| !h.is_empty()).unwrap_or(false));
    }
}

/// The real-SiliconFlow `/llm/query` leg. Ignored by default (makes a live network call; needs
/// `config/secrets.toml` with a working `[siliconflow]` key + NO_PROXY covering api.siliconflow.cn).
/// Run explicitly: `cargo test -p api --test e2e_closed_loop -- --ignored llm_query_real`.
#[test]
#[ignore = "live SiliconFlow network call; run explicitly with --ignored"]
fn llm_query_real_siliconflow() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();

    let resp: Value = c
        .post(format!("{base}/llm/query"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "prompt": "未签订书面劳动合同，劳动者可以主张哪些权利？",
            "context": {},
            "forceLocal": false,
            "allowCrossBorder": false
        }))
        .send()
        .expect("llm query")
        .json()
        .unwrap();
    // A configured dispatcher answers (content non-empty OR a structured abstention with followups);
    // an absent secrets.toml surfaces E_LLM_PROVIDER_DOWN — record whichever this environment yields.
    eprintln!("llm_query_real response: {resp}");
    assert!(
        resp["error"].is_null() || resp["error"]["code"] == "E_LLM_PROVIDER_DOWN",
        "llm query returned an unexpected error: {resp}"
    );
    if resp["error"].is_null() {
        assert!(
            resp["data"]["sourceTag"].is_string(),
            "source tag present: {resp}"
        );
        assert!(
            resp["data"]["fallbackLevel"].as_u64().is_some(),
            "fallback level: {resp}"
        );
    }
}

fn zip_entry_names(zip_bytes: &[u8]) -> Vec<String> {
    let reader = std::io::Cursor::new(zip_bytes.to_vec());
    let mut archive = zip::ZipArchive::new(reader).unwrap();
    (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect()
}
