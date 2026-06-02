//! `/diagnose` route integration test (prd/04 §4.1, module M1).
//!
//! Boots the in-process app with the REAL R1a services (incl. the deterministic
//! [`rule_engine::DiagnosisEngine`]) and drives the diagnosis surface over HTTP + Bearer exactly as
//! the Flutter parent would:
//! - `GET  /diagnose/tree`  returns the 20-branch root question.
//! - `POST /diagnose/step`  advances one question.
//! - `POST /diagnose`       runs the full deterministic diagnosis, persists the REAL subcategory
//!   onto a case (replacing `LD-00-00`), moves it draft -> diagnosed, and audits it.
//! - an incomplete path returns the structured `out_of_scope` object (abstention, never guessing).
//! - creating a case WITH an answer path diagnoses at creation time.

use std::time::Duration;

use api::{router, AppState};
use serde_json::{json, Value};
use stuchka_core::BootServices;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn spawn_app() -> String {
    let blob_dir = std::env::temp_dir().join(format!("stuchka-diag-{}", uuid::Uuid::now_v7()));
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
fn diagnose_surface_runs_the_real_engine() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let base = rt.block_on(spawn_app());
    let c = client();

    // ---- 1. GET /diagnose/tree → root question, 20 branches. ----
    let tree: Value = c
        .get(format!("{base}/diagnose/tree"))
        .bearer_auth(TOKEN)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(tree["error"].is_null(), "tree error: {tree}");
    assert_eq!(tree["data"]["nodeId"].as_str(), Some("root"));
    assert_eq!(
        tree["data"]["answers"].as_array().map(|a| a.len()),
        Some(20),
        "root has 20 category branches"
    );

    // ---- 2. POST /diagnose/step (root, wage) → asks the wage question. ----
    let step: Value = c
        .post(format!("{base}/diagnose/step"))
        .bearer_auth(TOKEN)
        .json(&json!({ "nodeId": "root", "answerValue": "wage" }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(step["data"]["step"].as_str(), Some("ask"), "got {step}");
    assert!(!step["data"]["question"]["answers"]
        .as_array()
        .unwrap()
        .is_empty());

    // ---- 3. Create a case (draft, placeholder category surfaced as null). ----
    let create: Value = c
        .post(format!("{base}/case"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "identityType": "standard_full_time",
            "province": "44",
            "city": "4401",
            "caseOccurredAt": "2026-01-02",
            "disputeSubtype": "social_ins_arrears",
            "firstDescription": "公司拖欠工资三个月。",
            "kbVersionHash": ""
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    let case_id = create["data"]["id"].as_str().unwrap().to_string();
    assert!(
        create["data"]["disputeCategoryId"].is_null(),
        "pre-diagnosis category is null: {create}"
    );
    assert_eq!(create["data"]["status"].as_str(), Some("draft"));

    // ---- 4. POST /diagnose with a completed path → determinate LD-03-01, persists onto the case. ----
    let diag: Value = c
        .post(format!("{base}/diagnose"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "identityType": "standard_full_time",
            "disputeSubtype": "social_ins_arrears",
            "answerPath": ["wage", "arrears"],
            "caseId": case_id,
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(diag["error"].is_null(), "diagnose error: {diag}");
    assert_eq!(diag["data"]["status"].as_str(), Some("ok"));
    assert_eq!(diag["data"]["disputeCategory"].as_str(), Some("LD-03-01"));
    assert_eq!(diag["data"]["coverageTier"].as_str(), Some("make_deep"));
    assert_eq!(diag["data"]["coverageTag"].as_str(), Some("exact"));
    assert_eq!(
        diag["data"]["lawRef"].as_str(),
        Some("law:中华人民共和国劳动法/v2018-12-29/§50")
    );
    // recommended_procedures 主-并行-备用 present.
    assert_eq!(
        diag["data"]["recommendedProcedures"]["main"].as_str(),
        Some("arbitration")
    );
    assert!(!diag["data"]["nextActions"].as_array().unwrap().is_empty());

    // ---- 5. The case now carries the REAL subcategory + diagnosed status. ----
    let agg: Value = c
        .get(format!("{base}/case/{case_id}"))
        .bearer_auth(TOKEN)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(
        agg["data"]["disputeCategoryId"].as_str(),
        Some("LD-03-01"),
        "case carries the diagnosed subcategory: {agg}"
    );
    assert_eq!(agg["data"]["status"].as_str(), Some("diagnosed"));

    // ---- 6. The diagnosis was audited (ai_diagnosis_output) in the independent audit.sqlite. ----
    let audit: Value = c
        .get(format!("{base}/audit"))
        .query(&[("caseId", case_id.as_str())])
        .bearer_auth(TOKEN)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(audit["error"].is_null(), "audit query error: {audit}");
    let entries = audit["data"].as_array().expect("audit data is an array");
    assert!(!entries.is_empty(), "audit has case-scoped entries");
    assert!(
        entries.iter().any(|e| e["why"].as_str() == Some("ai_diagnosis_output")),
        "audit must record ai_diagnosis_output: {audit}"
    );

    // ---- 7. Incomplete path → structured out_of_scope (abstention, never a guess). ----
    let oos: Value = c
        .post(format!("{base}/diagnose"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "identityType": "standard_full_time",
            "disputeSubtype": "social_ins_arrears",
            "answerPath": ["work_injury"],
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(oos["data"]["status"].as_str(), Some("out_of_scope"));
    assert!(!oos["data"]["reasons"].as_array().unwrap().is_empty());

    // ---- 8. Create a case WITH an answer path → diagnosed at creation time. ----
    let create2: Value = c
        .post(format!("{base}/case"))
        .bearer_auth(TOKEN)
        .json(&json!({
            "identityType": "standard_full_time",
            "province": "44",
            "city": "4401",
            "caseOccurredAt": "2026-01-02",
            "disputeSubtype": "social_ins_arrears",
            "firstDescription": "未签书面劳动合同，主张二倍工资。",
            "kbVersionHash": "",
            "answerPath": ["contract_formation", "no_written"]
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(
        create2["data"]["disputeCategoryId"].as_str(),
        Some("LD-01-01"),
        "diagnosed at creation: {create2}"
    );
    assert_eq!(create2["data"]["status"].as_str(), Some("diagnosed"));
}
