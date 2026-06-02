//! AU-08..12 — OpenTimestamps binding + daily-hash acceptance (data/04 §4.11.2, R1a必死).

mod common;

use audit::{
    compute_daily_sha256, verify_binding, AuditReason, DailyDigest, MockOtsClient, OtsClient,
    TrustState,
};
use chrono::Utc;
use common::*;

fn today() -> chrono::NaiveDate {
    Utc::now().date_naive()
}

fn sample_digest() -> DailyDigest {
    DailyDigest {
        date: chrono::NaiveDate::from_ymd_opt(2026, 5, 12).unwrap(),
        last_record_hash: "a".repeat(64),
        record_count: 123,
    }
}

/// AU-08: compute_daily_sha256 is deterministic for a fixed digest.
#[test]
fn au_08_daily_sha256_deterministic() {
    let d = sample_digest();
    let a = compute_daily_sha256(&d);
    let b = compute_daily_sha256(&sample_digest());
    assert_eq!(a, b);
}

/// AU-09: after anchor_to_ots, audit_anchor.ots_receipt and ots_anchored_at are both non-null.
#[tokio::test]
async fn au_09_ots_receipt_persisted_is_r1a_done() {
    let log = test_audit_log_with_mock_ots().await;
    append_case(&log, AuditReason::AiDiagnosisOutput).await;
    let digest = log.compute_daily_hash(today()).await.unwrap();
    let client = MockOtsClient::new();
    log.anchor_to_ots(&client, &digest).await.unwrap();
    let anchor = log.fetch_anchor(today()).await.unwrap().unwrap();
    assert!(
        anchor.ots_receipt.is_some() && anchor.ots_anchored_at.is_some(),
        "R1a done: OTS receipt + timestamp must be persisted"
    );
}

/// AU-10: a receipt must fail binding verification against the wrong hash.
#[tokio::test]
async fn au_10_ots_binding_rejects_wrong_hash() {
    let digest = sample_digest();
    let client = MockOtsClient::new();
    let receipt = client.stamp(&compute_daily_sha256(&digest)).await.unwrap();
    let wrong = [0u8; 32];
    assert!(verify_binding(&receipt, &wrong).is_err());
    assert!(verify_binding(&receipt, &compute_daily_sha256(&digest)).is_ok());
}

/// AU-11: with no OTS anchor, the trust aggregation reports Layer 2 = false.
#[tokio::test]
async fn au_11_missing_ots_layer2_false() {
    let log = test_audit_log().await;
    append_case(&log, AuditReason::DocumentFinalized).await;
    let t: TrustState = log.trust_state_for::<MockOtsClient>(today()).await.unwrap();
    assert!(t.chain_ok, "chain intact");
    assert!(!t.ots_verified, "Layer 2 false without anchor");
    assert!(!t.is_court_admissible());

    // After anchoring, Layer 2 flips true.
    let digest = log.compute_daily_hash(today()).await.unwrap();
    log.anchor_to_ots(&MockOtsClient::new(), &digest)
        .await
        .unwrap();
    let t2 = log.trust_state_for::<MockOtsClient>(today()).await.unwrap();
    assert!(t2.ots_verified, "Layer 2 true after anchor");
    assert!(t2.is_court_admissible());
}

/// AU-12: changing a record on the day changes the recomputed daily hash and invalidates the old
/// receipt's binding.
#[tokio::test]
async fn au_12_record_change_invalidates_daily_hash() {
    let log = test_audit_log().await;
    append_case(&log, AuditReason::DocumentFinalized).await;

    let digest_before = log.compute_daily_hash(today()).await.unwrap();
    let d_before = compute_daily_sha256(&digest_before);
    // Mint a receipt for the pre-tamper hash.
    let old_receipt = MockOtsClient::new().stamp(&d_before).await.unwrap();
    assert!(verify_binding(&old_receipt, &d_before).is_ok());

    tamper_what_payload_at_seq(&log, 1).await;

    let digest_after = log.compute_daily_hash(today()).await.unwrap();
    let d_after = compute_daily_sha256(&digest_after);
    assert_ne!(
        d_before, d_after,
        "tampering a record must change the daily hash"
    );
    // The old receipt no longer binds to the new daily hash.
    assert!(verify_binding(&old_receipt, &d_after).is_err());
}
