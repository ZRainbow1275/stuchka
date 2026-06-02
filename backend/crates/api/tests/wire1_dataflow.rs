//! wire1 end-to-end data-flow integration test (STEP wire1).
//!
//! Boots an in-process app with the REAL R1a module services (in-memory db + audit, real
//! rule-engine + KB index + hsd + age store), binds it to a loopback port, and drives it over
//! HTTP + Bearer exactly as the Flutter parent would. It exercises the full M1/M2/M9/M5/M6 chain:
//!
//! create case → add 2 facts → upload an evidence whose metadata carries a phone number (triggers
//! the hsd high-sensitive path + age encryption) → `/compute/run` severance for a known
//! wage/tenure and assert the exact Decimal result → `/deadline/run` and assert
//! `manual_confirm_required` → `/law-ref` search hits a seeded clause. Finally it asserts the
//! independent `audit.sqlite` gained entries (via `GET /audit?case_id=`) and that the chain
//! verifies (the boot path runs `verify_on_startup`, and the chain-broken flag gates every write).

use std::time::Duration;

use api::{router, AppState};
use serde_json::{json, Value};
use stuchka_core::BootServices;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Spawn the in-process app on a loopback port using in-memory services; returns the base URL.
async fn spawn_app() -> String {
    let blob_dir = std::env::temp_dir().join(format!("stuchka-wire1-{}", uuid::Uuid::now_v7()));
    let services = BootServices::initialize_in_memory(blob_dir)
        .await
        .expect("init in-memory services");
    let state = AppState::from_boot(TOKEN, services);
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
        .timeout(Duration::from_secs(15))
        .no_proxy()
        .build()
        .expect("build client")
}

#[test]
fn wire1_full_dataflow_over_http_bearer() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();

    // ---- 1. Create a case (M1) — freezes the live KB hash (INV-04). ----
    let create_case = c
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
        .expect("create case");
    assert_eq!(create_case.status().as_u16(), 201, "POST /case => 201");
    let case_body: Value = create_case.json().unwrap();
    assert!(
        case_body["error"].is_null(),
        "case create error: {case_body}"
    );
    let case_id = case_body["data"]["id"]
        .as_str()
        .expect("case id")
        .to_string();
    // The frozen kb hash is the server's live 64-hex global_hash, not the client's guess.
    let frozen = case_body["data"]["kbVersionHash"].as_str().unwrap();
    assert_eq!(frozen.len(), 64, "frozen kb hash is 64-hex: {frozen}");
    assert_ne!(frozen, "client-thinks-this");

    // ---- 2. Add two facts (M1). ----
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
    // The fact list must now hold two facts.
    let facts: Value = c
        .get(format!("{base}/case/{case_id}/fact"))
        .bearer_auth(TOKEN)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(
        facts["data"].as_array().map(|a| a.len()),
        Some(2),
        "two facts persisted: {facts}"
    );

    // ---- 3. Upload an evidence whose metadata carries a phone number (hsd high-sensitive). ----
    let metadata = json!({
        "category": "recording",
        "collected_at": "2026-01-03T10:00:00Z",
        "device_id": "dev-sha-1",
        "linked_fact_ids": []
    })
    .to_string();
    // The phone number 13800138000 is a hsd strong signal -> is_high_sensitive + age encryption.
    let form = reqwest::blocking::multipart::Form::new()
        .text("metadata", metadata)
        .part(
            "file",
            reqwest::blocking::multipart::Part::bytes(
                "录音转写：我的联系电话是 13800138000，请尽快处理欠薪。"
                    .as_bytes()
                    .to_vec(),
            )
            .file_name("recording.txt"),
        );
    let ev = c
        .post(format!("{base}/case/{case_id}/evidence"))
        .bearer_auth(TOKEN)
        .multipart(form)
        .send()
        .expect("upload evidence");
    assert_eq!(ev.status().as_u16(), 201, "POST evidence => 201");
    let ev_body: Value = ev.json().unwrap();
    assert!(ev_body["error"].is_null(), "evidence error: {ev_body}");
    assert_eq!(
        ev_body["data"]["isHighSensitive"], true,
        "phone number must trigger hsd high-sensitive: {ev_body}"
    );
    // The high-sensitive file is stored as an age blob (path under the blob dir, not the raw file).
    let stored_path = ev_body["data"]["filePath"].as_str().unwrap();
    assert!(
        stored_path.contains("blb_"),
        "high-sensitive evidence stored as an age blob: {stored_path}"
    );
    assert!(
        !ev_body["data"]["piiHits"].as_array().unwrap().is_empty(),
        "pii hits surfaced: {ev_body}"
    );
    // The five-dim score is in range.
    let score = ev_body["data"]["effectiveScore"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&score), "score in range: {score}");

    // ---- 4. /compute/run severance for a known wage/tenure → exact Decimal. ----
    // Guangdong (44), monthly wage 8000 (below the 3x social-avg cap), tenure 2018-01-01..2026-01-01
    // = exactly 8 years → severance = 8000 * 8 = 64000.00.
    let compute = c
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
        .expect("compute run");
    assert_eq!(compute.status().as_u16(), 200, "POST /compute/run => 200");
    let comp_body: Value = compute.json().unwrap();
    assert!(comp_body["error"].is_null(), "compute error: {comp_body}");
    // Money is serialized as a STRING by rust_decimal's serde (NEVER f64). Assert the exact
    // monetary VALUE (8000 * 8 years = 64000) numerically — the engine's `round_dp(2)` may emit a
    // scale-0 or scale-2 string depending on the value's fractional digits.
    let expected = rust_decimal::Decimal::from(64000);
    let result0 = &comp_body["data"]["results"][0];
    assert_eq!(result0["scenario"], "severance");
    let amount_str = result0["amount"]
        .as_str()
        .expect("amount serialized as string");
    let amount: rust_decimal::Decimal = amount_str.parse().expect("amount parses as Decimal");
    assert_eq!(
        amount, expected,
        "severance 8000 * 8 years = 64000 (got {amount_str})"
    );
    assert_eq!(result0["outcome"]["status"], "ok");
    // preTax is the same monetary value.
    let pre_tax = comp_body["data"]["preTax"].as_str().expect("preTax string");
    let pre_tax_dec: rust_decimal::Decimal = pre_tax.parse().expect("preTax parses as Decimal");
    assert_eq!(pre_tax_dec, expected, "aggregate preTax == 64000");

    // ---- 5. /deadline/run → manual_confirm_required true (INV-08). ----
    let deadline = c
        .post(format!("{base}/deadline/run"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "caseId": case_id,
            "province": "44",
            "city": "4401",
            "events": [{
                "caseOccurredAt": "2026-01-02",
                "kind": "arbitration_general",
                "laborRelationshipActive": false,
                "asOf": "2026-03-01"
            }]
        }))
        .send()
        .expect("deadline run");
    assert_eq!(deadline.status().as_u16(), 200, "POST /deadline/run => 200");
    let dl_body: Value = deadline.json().unwrap();
    assert!(dl_body["error"].is_null(), "deadline error: {dl_body}");
    let item0 = &dl_body["data"]["items"][0];
    assert_eq!(
        item0["manualConfirmRequired"], true,
        "INV-08: every Ok deadline requires manual confirm: {dl_body}"
    );
    assert_eq!(item0["outcome"]["status"], "ok");

    // ---- 6. /law-ref search hits a seeded clause. ----
    let search: Value = c
        .get(format!("{base}/law-ref"))
        .query(&[("q", "经济补偿 解除 工龄"), ("limit", "10")])
        .bearer_auth(TOKEN)
        .send()
        .expect("law-ref search")
        .json()
        .unwrap();
    assert!(search["error"].is_null(), "search error: {search}");
    let hits = search["data"].as_array().expect("hit array");
    assert!(
        !hits.is_empty(),
        "search must hit a seeded clause: {search}"
    );
    assert!(
        hits.iter().any(|h| h["stableId"]
            .as_str()
            .map(|s| s.contains("劳动合同法"))
            .unwrap_or(false)),
        "a 劳动合同法 clause is among the hits: {search}"
    );

    // ---- 7. The independent audit.sqlite gained entries; the chain verifies. ----
    // GET /audit?case_id returns the case-scoped projection; it only succeeds when the startup
    // chain verification passed (the handler 500s on a broken chain), so a non-empty 200 proves
    // both that entries were written AND that verify_chain holds.
    let audit: Value = c
        .get(format!("{base}/audit"))
        .query(&[("caseId", case_id.as_str())])
        .bearer_auth(TOKEN)
        .send()
        .expect("audit query")
        .json()
        .unwrap();
    assert!(audit["error"].is_null(), "audit query error: {audit}");
    let entries = audit["data"].as_array().expect("audit entries");
    // case create (1) + 2 facts (2) + evidence upload (1) + hsd-detected (1) = >= 4 case-scoped.
    assert!(
        entries.len() >= 4,
        "audit.sqlite gained case-scoped entries: got {} ({audit})",
        entries.len()
    );
    // The chain projection is seq-ascending and links prev->record (rebuildable).
    let mut last_seq = -1;
    for e in entries {
        let seq = e["seq"].as_i64().unwrap();
        assert!(seq > last_seq, "entries are seq-ascending");
        last_seq = seq;
        assert!(e["recordHash"]
            .as_str()
            .map(|h| !h.is_empty())
            .unwrap_or(false));
    }
}
