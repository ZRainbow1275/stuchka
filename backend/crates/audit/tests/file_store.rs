//! On-disk audit.sqlite path (data/04 §4.3): real file, WAL, migrations, append + reopen + verify.

mod common;

use audit::AuditReason;
use common::{cid, user};
use serde_json::json;

/// The production `open_audit_log` path: create a file audit.sqlite, append across two opens, and
/// confirm the chain persists and verifies (models the §4.9 reopen-then-verify recovery flow).
#[tokio::test]
async fn file_audit_log_persists_and_verifies_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.sqlite");
    let key = [0x5Au8; 32];

    {
        let log = audit::open_audit_log(&path, &key).await.unwrap();
        log.append(
            user(),
            AuditReason::EvidenceUploaded,
            json!({"case_id": cid().to_string()}),
        )
        .await
        .unwrap();
        log.append(
            user(),
            AuditReason::DocumentFinalized,
            json!({"case_id": cid().to_string()}),
        )
        .await
        .unwrap();
    }

    // Reopen the same file (migrations are idempotent) and verify the chain survived.
    let log2 = audit::open_audit_log(&path, &key).await.unwrap();
    let checked = log2.verify_on_startup().await.unwrap();
    assert_eq!(checked, 2);

    let records = log2.query_by_case(cid()).await.unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].seq, 1);
    assert_eq!(records[1].seq, 2);
    assert_eq!(records[1].prev_hash, records[0].record_hash);
}
