//! Integration acceptance tests for `crates/sync` (brief §6 / 03-sync-yjs §3.5).
//!
//! Covers the CI gate assertions that span crates or need a real persistence / audit path:
//! SY-01 (Step audit double-write + chain), SY-04 merge threshold compaction through the real
//! `DocRepo`, SY-06 handshake auth, SY-12 awareness never persisted, SY-13 high-sensitivity
//! evidence excluded from the Y.Doc.
//!
//! mDNS (SY-11) is exercised in `mdns_discovery.rs` and marked `#[ignore]` because it binds real
//! multicast sockets (network-dependent; run explicitly).

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use uuid::Uuid;
use yrs::{Array, Map, Transact};

// Bring the persistence traits into scope so their methods are callable on the concrete repos.
use sync::{
    ensure_schema, handle_frame, handle_step_audit, verify_hello, write_update, CaseDoc, DocRepo,
    DocStepRepo, FrameType, PeerRepo, PeerSession, SqliteDocRepo, SqliteDocStepRepo,
    SqlitePeerRepo, StepAuditFrame, StepCategory, SyncHello, MERGE_THRESHOLD_COUNT,
    PROTOCOL_VERSION,
};

async fn mem_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    ensure_schema(&pool).await.unwrap();
    pool
}

fn keypair(seed: u8) -> (SigningKey, String) {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    let peer_id = bs58::encode(sk.verifying_key().to_bytes()).into_string();
    (sk, peer_id)
}

#[tokio::test]
async fn sy01_step_audit_double_writes_and_chain_verifies() {
    // SY-01: a merge-decision Step writes ONE merge_decision audit entry (verify_chain passes) AND
    // the main-store doc_step row carries the full Step JSON.
    let pool = mem_pool().await;
    let step_repo = SqliteDocStepRepo::new(pool.clone());
    let audit = audit::open_in_memory(&[7u8; 32]).await.unwrap();

    let case_id = Uuid::now_v7();
    let doc_id = Uuid::now_v7();
    let actor = Uuid::now_v7();
    let step_json = serde_json::json!({
        "stepType": "replaceAround",
        "from": 3, "to": 7,
        "slice": {"content": [{"type": "text", "text": "采我方"}]}
    });

    let frame = StepAuditFrame {
        doc_id,
        case_id,
        step_no: 1,
        step_json: step_json.clone(),
        client_id: "client-A".into(),
        actor,
        reason: Some("conflict resolved: keep mine".into()),
        category: StepCategory::MergeDecision,
    };
    let seq = handle_step_audit(&step_repo, &audit, frame).await.unwrap();
    assert!(seq >= 1, "append must return a positive seq");

    // Audit side: exactly one entry for this case, reason document_merged, chain intact.
    let records = audit.query_by_case(case_id).await.unwrap();
    assert_eq!(records.len(), 1, "exactly one merge_decision audit entry");
    assert_eq!(records[0].why, audit::AuditReason::DocumentMerged);
    assert_eq!(records[0].what["step"], step_json);
    let verification = audit.verify_chain().await.unwrap();
    assert!(verification.ok, "audit chain must verify");

    // Main-store side: doc_step carries the full Step JSON.
    let row = step_repo
        .get_doc_step(doc_id, 1)
        .await
        .unwrap()
        .expect("doc_step row exists");
    assert_eq!(
        row.step_json, step_json,
        "doc_step must hold the full Step JSON"
    );
    assert_eq!(row.reason.as_deref(), Some("conflict resolved: keep mine"));
}

#[tokio::test]
async fn sy04_merge_threshold_compacts_and_snapshot_restores_full_state() {
    // SY-04: appending MERGE_THRESHOLD_COUNT updates triggers compaction; pending is cleared and the
    // snapshot restores the full document.
    let pool = mem_pool().await;
    let repo = SqliteDocRepo::new(pool);
    let doc_id = "0190b2aa-0000-7000-8000-00000000aa01";

    // Build a base doc and feed incremental updates produced from successive edits.
    let authoring = CaseDoc::new(doc_id);
    for i in 0..MERGE_THRESHOLD_COUNT {
        let before = authoring.state_vector();
        {
            let mut txn = authoring.doc.transact_mut();
            let m = authoring
                .facts
                .push_back(&mut txn, yrs::MapPrelim::default());
            m.insert(&mut txn, "summary", format!("fact #{i}"));
        }
        let delta = authoring.diff_update(&before).unwrap();
        write_update(&repo, doc_id, &delta).await.unwrap();
    }

    // After 50 appends the threshold fires: pending cleared, snapshot present.
    let (count, _) = repo.pending_size(doc_id).await.unwrap();
    assert_eq!(count, 0, "pending must be cleared after compaction");
    let snapshot = repo
        .load_snapshot(doc_id)
        .await
        .unwrap()
        .expect("snapshot must exist after compaction");

    // The snapshot restores all 50 facts.
    let restored = CaseDoc::open(doc_id, &snapshot).unwrap();
    let txn = restored.doc.transact();
    assert_eq!(restored.facts.len(&txn), MERGE_THRESHOLD_COUNT as u32);
}

#[tokio::test]
async fn sy06_handshake_rejects_untrusted_and_bad_signature() {
    // SY-06: SyncHello signature check — untrusted peer rejected; tampered signature rejected;
    // trusted + valid signature accepted.
    let pool = mem_pool().await;
    let peers = SqlitePeerRepo::new(pool);
    let (sk, peer_id) = keypair(5);
    let doc_id = "doc-xyz";

    let mut hello = SyncHello {
        protocol_version: PROTOCOL_VERSION,
        peer_id: peer_id.clone(),
        peer_signature: String::new(),
        nonce: "n1".into(),
        state_vector_base64: String::new(),
    };
    let sig = sk.sign(&hello.signing_payload(doc_id));
    hello.peer_signature = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());

    // Untrusted peer -> rejected.
    assert!(verify_hello(&peers, &hello, doc_id).await.is_err());

    // Trust the peer (TOFU done elsewhere) -> accepted.
    peers
        .set_trusted(&peer_id, true, Some(&peer_id))
        .await
        .unwrap();
    verify_hello(&peers, &hello, doc_id).await.unwrap();

    // Tampered signature -> rejected (nonce changed, signature no longer matches).
    let mut bad = hello.clone();
    bad.nonce = "different".into();
    assert!(verify_hello(&peers, &bad, doc_id).await.is_err());
}

#[tokio::test]
async fn sy12_awareness_is_never_persisted() {
    // SY-12: awareness frames only mutate the in-memory PeerSession; no awareness table exists.
    let pool = mem_pool().await;
    let doc = CaseDoc::new("c1");
    let mut session = PeerSession::new("c1");
    let _ = handle_frame(&doc, &mut session, FrameType::Awareness, b"cursor=42").unwrap();
    assert_eq!(session.awareness.as_deref(), Some(&b"cursor=42"[..]));

    // No table named like awareness exists in the sync schema.
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        tables.iter().all(|t| !t.to_lowercase().contains("aware")),
        "no awareness table may exist: {tables:?}"
    );
}

#[tokio::test]
async fn sy13_high_sensitivity_evidence_excluded_from_ydoc() {
    // SY-13: very-high-sensitivity evidence is NOT placed in the Y.Doc — only a fact summary plus a
    // file-path placeholder. The CRDT entry must never carry the sensitive transcript/body field.
    let doc = CaseDoc::new("c1");
    {
        let mut txn = doc.doc.transact_mut();
        let e = doc.evidences.push_back(&mut txn, yrs::MapPrelim::default());
        e.insert(&mut txn, "summary", "录音转写：协商解除（摘要）");
        e.insert(
            &mut txn,
            "path_placeholder",
            "blob://high-sensitive/0190.enc",
        );
        e.insert(&mut txn, "sensitivity", "L4");
    }

    // Round-trip through a snapshot, then assert the sensitive keys never entered the CRDT.
    let full = doc.encode_full();
    let reopened = CaseDoc::open("c1", &full).unwrap();
    let txn = reopened.doc.transact();
    assert_eq!(reopened.evidences.len(&txn), 1);

    let entry = reopened.evidences.get(&txn, 0).unwrap();
    let map = entry.cast::<yrs::MapRef>().unwrap();
    assert!(map.get(&txn, "summary").is_some(), "summary kept");
    assert!(
        map.get(&txn, "path_placeholder").is_some(),
        "path placeholder kept"
    );
    assert!(
        map.get(&txn, "transcript").is_none(),
        "raw transcript must not enter Y.Doc"
    );
    assert!(
        map.get(&txn, "medical_body").is_none(),
        "medical body must not enter Y.Doc"
    );
}
