//! Shared test fixtures for the audit integration tests (data/04 §4.11 skeleton).
//!
//! `tamper_what_payload_at_seq` and `raw_delete_seq` need to bypass the append-only triggers to
//! simulate an attacker / disk-level corruption. SQLite triggers cannot be dropped on a live
//! connection without `DROP TRIGGER` (which itself is fine — the triggers guard DML, not DDL), so
//! the tamper fixture temporarily drops the triggers, mutates the row, and recreates them — this
//! models an out-of-band edit to the file that `verify_chain` must still catch.

#![allow(dead_code)]

use audit::{AuditLog, AuditReason, Subject};
use sqlx::Row;
use uuid::Uuid;

/// A fresh in-memory audit log with a fixed test key.
pub async fn test_audit_log() -> AuditLog {
    audit::open_in_memory(&[0x42u8; 32]).await.unwrap()
}

/// Same as [`test_audit_log`] — the OTS client is supplied per-call via `MockOtsClient`, so the
/// "with mock ots" fixture is just an alias kept for parity with the data/04 skeleton names.
pub async fn test_audit_log_with_mock_ots() -> AuditLog {
    test_audit_log().await
}

/// A stable user subject.
pub fn user() -> Subject {
    Subject::User {
        user_id: Uuid::parse_str("018f0000-0000-7000-8000-000000000001").unwrap(),
    }
}

/// A stable case id for `what.case_id`.
pub fn cid() -> Uuid {
    Uuid::parse_str("018f0000-0000-7000-8000-0000000000aa").unwrap()
}

/// Bypass the append-only triggers and corrupt `what_payload` at `seq` (models an out-of-band
/// edit). `verify_chain` must still detect the break because `record_hash` no longer matches.
pub async fn tamper_what_payload_at_seq(log: &AuditLog, seq: i64) {
    let pool = log.pool();
    sqlx::query("DROP TRIGGER IF EXISTS audit_no_update")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER IF EXISTS audit_no_delete")
        .execute(pool)
        .await
        .unwrap();

    // Flip a byte in the stored ciphertext without updating record_hash.
    let current: Vec<u8> = sqlx::query("SELECT what_payload FROM audit_log WHERE seq = ?")
        .bind(seq)
        .fetch_one(pool)
        .await
        .unwrap()
        .get("what_payload");
    let mut corrupted = current.clone();
    if let Some(b) = corrupted.first_mut() {
        *b ^= 0xFF;
    } else {
        corrupted.push(0xAA);
    }
    sqlx::query("UPDATE audit_log SET what_payload = ? WHERE seq = ?")
        .bind(&corrupted)
        .bind(seq)
        .execute(pool)
        .await
        .unwrap();

    recreate_triggers(pool).await;
}

/// Bypass the triggers and delete `seq` (used to model a chain hole, distinct from AU-05 which
/// asserts the trigger rejects a *normal* delete).
pub async fn raw_delete_seq(log: &AuditLog, seq: i64) {
    let pool = log.pool();
    sqlx::query("DROP TRIGGER IF EXISTS audit_no_update")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER IF EXISTS audit_no_delete")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM audit_log WHERE seq = ?")
        .bind(seq)
        .execute(pool)
        .await
        .unwrap();
    recreate_triggers(pool).await;
}

async fn recreate_triggers(pool: &sqlx::SqlitePool) {
    sqlx::query(
        "CREATE TRIGGER audit_no_update BEFORE UPDATE ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER audit_no_delete BEFORE DELETE ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Try to DELETE a row through the normal (trigger-guarded) path; returns the SQL error.
pub async fn delete_through_trigger(log: &AuditLog, seq: i64) -> sqlx::Error {
    sqlx::query("DELETE FROM audit_log WHERE seq = ?")
        .bind(seq)
        .execute(log.pool())
        .await
        .unwrap_err()
}

/// Try to UPDATE a row through the normal (trigger-guarded) path; returns the SQL error.
pub async fn update_through_trigger(log: &AuditLog, seq: i64) -> sqlx::Error {
    sqlx::query("UPDATE audit_log SET why = 'x' WHERE seq = ?")
        .bind(seq)
        .execute(log.pool())
        .await
        .unwrap_err()
}

/// Convenience: append a case-scoped record.
pub async fn append_case(log: &AuditLog, why: AuditReason) -> i64 {
    log.append(
        user(),
        why,
        serde_json::json!({ "case_id": cid().to_string() }),
    )
    .await
    .unwrap()
}
