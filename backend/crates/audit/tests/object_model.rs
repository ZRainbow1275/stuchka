//! OM-07 (data/01 §1.9) + sync step-audit (backend/03 §3.5) — the audit-side assertions.
//!
//! The main-store `doc_step` half of the sync test belongs to `crates/sync`; here we assert the
//! independent audit library half: a merge produces one `merge_decision`-category record and the
//! chain still verifies.

mod common;

use audit::{AuditReason, Subject};
use common::*;
use data_model::AuditCategory;
use serde_json::json;
use uuid::Uuid;

/// OM-07: `evidence.status='quarantined'` is mirrored by an `EvidenceQuarantined` audit record
/// (category state_change) that carries the case soft-reference and verifies.
#[tokio::test]
async fn om_07_evidence_quarantined_is_audited() {
    let log = test_audit_log().await;
    let case = cid();
    let evidence_id = Uuid::now_v7();
    let seq = log
        .append(
            user(),
            AuditReason::EvidenceQuarantined,
            json!({
                "case_id": case.to_string(),
                "evidence_id": evidence_id.to_string(),
                "from": "uploaded",
                "to": "quarantined",
                "reason": "INV-09 forged-evidence quarantine"
            }),
        )
        .await
        .unwrap();

    assert_eq!(
        AuditReason::EvidenceQuarantined.category(),
        AuditCategory::StateChange
    );

    let records = log.query_by_case(case).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].seq, seq);
    assert!(matches!(records[0].why, AuditReason::EvidenceQuarantined));
    assert_eq!(records[0].case_id, Some(case));

    let v = log.verify_chain().await.unwrap();
    assert!(v.ok);
}

/// sync step-audit (backend/03 §3.5, audit half): a ProseMirror Step conflict resolution writes one
/// `merge_decision`-category record to the independent audit library, and verify_chain passes.
#[tokio::test]
async fn sync_step_audit_writes_merge_decision_record() {
    let log = test_audit_log().await;
    let case = cid();

    // Two contributors edited the same block; the merge decision is audited (DocumentMerged ->
    // merge_decision category).
    let who = Subject::System {
        component: "yjs_merge".into(),
    };
    let seq = log
        .append(
            who,
            AuditReason::DocumentMerged,
            json!({
                "case_id": case.to_string(),
                "doc_id": Uuid::now_v7().to_string(),
                "resolution": "yjs_auto_merge",
                "conflicting_steps": 2
            }),
        )
        .await
        .unwrap();
    assert_eq!(seq, 1);

    // exactly one merge_decision-category record exists in the independent library
    let records = log.query_by_case(case).await.unwrap();
    let merge_records: Vec<_> = records
        .iter()
        .filter(|r| r.why.category() == AuditCategory::MergeDecision)
        .collect();
    assert_eq!(merge_records.len(), 1);

    let v = log.verify_chain().await.unwrap();
    assert!(v.ok, "chain must verify after a merge-decision audit write");
}

/// Startup verification gate (data/04 §4.9): a broken chain surfaces as ChainBroken so the caller
/// refuses new writes (C-C-15).
#[tokio::test]
async fn startup_verify_rejects_broken_chain() {
    let log = test_audit_log().await;
    for _ in 0..3 {
        append_case(&log, AuditReason::UserConfirmFact).await;
    }
    // clean startup ok
    let checked = log.verify_on_startup().await.unwrap();
    assert_eq!(checked, 3);

    tamper_what_payload_at_seq(&log, 2).await;
    let err = log.verify_on_startup().await.unwrap_err();
    assert_eq!(err.error_code(), data_model::ErrorCode::AuditChainBroken);
}
