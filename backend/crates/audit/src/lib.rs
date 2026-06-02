//! `audit` — independent `audit.sqlite` + who/when/why/what four-tuple + prev_hash->record_hash
//! chain + ChaCha20-Poly1305 (random per-segment nonce, W5) + OpenTimestamps anchor + startup
//! chain verification (master-index §0.2 D3, data/04 全文).
//!
//! Module map (data/04 file layout, I7 — backend's `append_log`/`ots`/`four_w`/`store` names are
//! re-exported at the crate root for compatibility):
//! - [`subject`]     — §4.2.1 `Subject` (the `who`).
//! - [`reason`]      — §4.4 `AuditReason` (~40 trigger points) + `AuditReason::category`.
//! - [`hash_chain`]  — §4.3.1 `compute_record_hash` + `GENESIS_PREV_HASH`.
//! - [`cipher`]      — §4.5 ChaCha20-Poly1305 segment cipher (random per-segment nonce, W5).
//! - [`daily_hash`]  — §4.6.1 daily digest + deterministic `compute_daily_sha256`.
//! - [`ots`]         — §4.6.2 `OtsClient` trait + `MockOtsClient` + `CliOtsClient` seam (W8).
//! - [`trust`]       — §4.8 three-layer court-admissibility aggregation (C-C-15).
//! - [`schema`]      — §4.4 per-`why` `what` validation.
//! - [`migrate`]     — independent migration runner for `audit.sqlite`.
//! - [`store`]       — §4.7 `AuditLog` (`append` / `query_by_case` / `verify_chain` / anchor).
//!
//! D3 boundary: this crate has ZERO dependency on `crates/db`. It depends down only on
//! `data-model` (shared `AuditCategory` / `AuditSubject` / `ErrorCode`) and `crypto` (the
//! `hkdf_subkey(DEK, AUDIT_INFO)` audit-key derivation, C-3).

pub mod cipher;
pub mod daily_hash;
pub mod error;
pub mod hash_chain;
pub mod migrate;
pub mod ots;
pub mod reason;
pub mod schema;
pub mod store;
pub mod subject;
pub mod trust;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous};
use std::path::Path;
use std::str::FromStr;

// Crate-root re-exports (data/04 names + backend compat).
pub use cipher::AuditCipher;
pub use daily_hash::{compute_daily_hash, compute_daily_sha256, daily_sha256_hex, DailyDigest};
pub use error::{AnchorError, AuditError, CipherError, DailyHashError, OtsError};
pub use hash_chain::{compute_record_hash, GENESIS_PREV_HASH};
pub use ots::{verify_binding, CliOtsClient, MockOtsClient, OtsClient, OTS_RECEIPT_MAGIC};
pub use reason::AuditReason;
pub use store::{AnchorRow, AuditLog, AuditRecord, ChainVerification, RawRecord};
pub use subject::Subject;
pub use trust::{Admissibility, TrustState};

/// HKDF `info` for the audit key — re-exported from `crypto` so callers have one source of truth
/// (`audit_key = crypto::hkdf_subkey(DEK, AUDIT_INFO)`, backend/04 §4.8 / C-3).
pub use crypto::AUDIT_INFO;

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "audit";

/// Derive the 32-byte audit key from the session DEK (`crypto::hkdf_subkey(DEK, AUDIT_INFO)`).
///
/// The DEK never enters this crate as raw bytes from untrusted callers — pass
/// `session_dek.expose()` from `crypto::SessionDek` at the unlock boundary.
#[must_use]
pub fn derive_audit_key(dek: &[u8; 32]) -> [u8; 32] {
    crypto::hkdf_subkey(dek, AUDIT_INFO)
}

/// Open the independent `audit.sqlite` at `path` (creating it if missing), with WAL +
/// `synchronous=FULL` (data/04 §4.3), and run migrations. Returns a ready [`AuditLog`].
pub async fn open_audit_log(
    path: impl AsRef<Path>,
    audit_key: &[u8; 32],
) -> Result<AuditLog, AuditError> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;
    migrate::run_migrations(&pool).await?;
    Ok(AuditLog::new(pool, audit_key))
}

/// Open an in-memory audit store (tests / ephemeral) with migrations applied. A single shared
/// connection keeps the schema alive for the pool lifetime.
pub async fn open_in_memory(audit_key: &[u8; 32]) -> Result<AuditLog, AuditError> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    migrate::run_migrations(&pool).await?;
    Ok(AuditLog::new(pool, audit_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(CRATE_NAME, "audit");
    }

    #[test]
    fn audit_key_is_hkdf_of_dek() {
        let dek = [0x11u8; 32];
        let k = derive_audit_key(&dek);
        assert_eq!(k, crypto::hkdf_subkey(&dek, crypto::AUDIT_INFO));
        // different DEK -> different audit key
        assert_ne!(k, derive_audit_key(&[0x22u8; 32]));
    }

    #[tokio::test]
    async fn open_in_memory_is_empty_and_verifies() {
        let log = open_in_memory(&[7u8; 32]).await.unwrap();
        let v = log.verify_chain().await.unwrap();
        assert!(v.ok);
        assert_eq!(v.checked, 0);
        assert_eq!(v.broken_at_seq, None);
    }
}
