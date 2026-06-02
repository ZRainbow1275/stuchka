//! AU-01..07 — chain hash acceptance (data/04 §4.11.1).

mod common;

use audit::{AuditReason, GENESIS_PREV_HASH};
use common::*;
use serde_json::json;

/// AU-01: the first record's prev_hash is the 64-zero genesis.
#[tokio::test]
async fn au_01_first_prev_hash_is_genesis() {
    let log = test_audit_log().await;
    let s1 = append_case(&log, AuditReason::EvidenceUploaded).await;
    assert_eq!(s1, 1);
    let r1 = log.fetch_raw(s1).await.unwrap();
    assert_eq!(r1.prev_hash, GENESIS_PREV_HASH);
}

/// AU-02: record N's prev_hash equals record N-1's record_hash.
#[tokio::test]
async fn au_02_prev_links_to_previous_record_hash() {
    let log = test_audit_log().await;
    let s1 = append_case(&log, AuditReason::EvidenceUploaded).await;
    let s2 = append_case(&log, AuditReason::EvidenceParsed).await;
    let r1 = log.fetch_raw(s1).await.unwrap();
    let r2 = log.fetch_raw(s2).await.unwrap();
    assert_eq!(r2.prev_hash, r1.record_hash);
}

/// AU-03: a clean chain verifies ok with broken_at_seq None.
#[tokio::test]
async fn au_03_clean_chain_verifies() {
    let log = test_audit_log().await;
    for _ in 0..5 {
        append_case(&log, AuditReason::UserConfirmFact).await;
    }
    let v = log.verify_chain().await.unwrap();
    assert!(v.ok);
    assert_eq!(v.checked, 5);
    assert_eq!(v.broken_at_seq, None);
}

/// AU-04: tampering a row's what_payload breaks the chain at that seq.
#[tokio::test]
async fn au_04_tamper_breaks_chain() {
    let log = test_audit_log().await;
    for _ in 0..5 {
        append_case(&log, AuditReason::UserConfirmFact).await;
    }
    tamper_what_payload_at_seq(&log, 3).await;
    let v = log.verify_chain().await.unwrap();
    assert!(!v.ok);
    assert_eq!(v.broken_at_seq, Some(3));
}

/// AU-05: UPDATE and DELETE through the normal path are rejected by the append-only triggers.
#[tokio::test]
async fn au_05_append_only_rejects_update_and_delete() {
    let log = test_audit_log().await;
    append_case(&log, AuditReason::CaseFreeze).await;

    let del_err = delete_through_trigger(&log, 1).await;
    assert!(
        del_err.to_string().contains("append-only"),
        "delete err was: {del_err}"
    );

    let upd_err = update_through_trigger(&log, 1).await;
    assert!(
        upd_err.to_string().contains("append-only"),
        "update err was: {upd_err}"
    );
}

/// AU-06: seq is strictly increasing 1..=K with no gaps.
#[tokio::test]
async fn au_06_seq_strictly_increments() {
    let log = test_audit_log().await;
    let mut seqs = Vec::new();
    for _ in 0..8 {
        seqs.push(append_case(&log, AuditReason::EvidenceScored).await);
    }
    assert_eq!(seqs, (1..=8).collect::<Vec<i64>>());
}

/// AU-07: record_hash carries a UNIQUE constraint. Two appends of identical (who, why, what) still
/// produce distinct record_hash because each segment uses a fresh random nonce (W5), so the chain
/// never violates the constraint in normal operation; a forced duplicate insert is rejected.
#[tokio::test]
async fn au_07_record_hash_unique() {
    let log = test_audit_log().await;
    let s1 = log
        .append(
            user(),
            AuditReason::EvidenceUploaded,
            json!({"case_id": cid().to_string()}),
        )
        .await
        .unwrap();
    let s2 = log
        .append(
            user(),
            AuditReason::EvidenceUploaded,
            json!({"case_id": cid().to_string()}),
        )
        .await
        .unwrap();
    let r1 = log.fetch_raw(s1).await.unwrap();
    let r2 = log.fetch_raw(s2).await.unwrap();
    assert_ne!(
        r1.record_hash, r2.record_hash,
        "random per-segment nonces keep record_hash distinct"
    );

    // A direct duplicate insert of an existing record_hash must hit the UNIQUE constraint.
    let dup = sqlx::query(
        "INSERT INTO audit_log \
         (prev_hash, record_hash, who_kind, who_payload, who_nonce, when_ts, why, what_payload, nonce, schema_version) \
         VALUES (?, ?, 'user', x'00', x'000000000000000000000000', 1, 'case_freeze', x'00', x'000000000000000000000000', 1)",
    )
    .bind(&r2.record_hash)
    .bind(&r1.record_hash) // collide with an existing record_hash
    .execute(log.pool())
    .await;
    assert!(dup.is_err(), "duplicate record_hash must violate UNIQUE");
}

/// Verify chain after a raw out-of-band delete: the prev-link breaks at the row whose prev_hash no
/// longer matches the (now-missing) predecessor's record_hash.
#[tokio::test]
async fn raw_delete_breaks_prev_link() {
    let log = test_audit_log().await;
    for _ in 0..4 {
        append_case(&log, AuditReason::UserConfirmFact).await;
    }
    raw_delete_seq(&log, 2).await;
    let v = log.verify_chain().await.unwrap();
    assert!(!v.ok);
    // seq 3 expected prev = genesis-chained hash from seq 1, but its stored prev_hash points at the
    // deleted seq 2's record_hash -> break detected at seq 3.
    assert_eq!(v.broken_at_seq, Some(3));
}
