//! Error types for `crates/audit`.
//!
//! The crate maps its failure modes onto the data-model error codes at the api boundary
//! (`E_AUDIT_CHAIN_BROKEN` for a broken chain on startup, `E_OTS_UPSTREAM` for anchor failures —
//! backend/01 §1.12); [`AuditError::error_code`] exposes that mapping.

use data_model::ErrorCode;

/// ChaCha20-Poly1305 segment cipher failure (§4.5).
#[derive(Debug, thiserror::Error)]
pub enum CipherError {
    /// AEAD seal/open failed (tag mismatch, wrong key, or tampered ciphertext).
    #[error("audit AEAD seal/open failed")]
    Aead,
    /// Stored nonce was not 12 bytes.
    #[error("audit nonce must be 12 bytes, got {0}")]
    BadNonceLen(usize),
}

/// Daily-hash computation failure (§4.6.1).
#[derive(Debug, thiserror::Error)]
pub enum DailyHashError {
    /// Underlying SQLite error.
    #[error("audit daily-hash db error: {0}")]
    Db(#[from] sqlx::Error),
}

/// OpenTimestamps anchoring failure (§4.6.2). Maps to `E_OTS_UPSTREAM` (502) at the api boundary.
#[derive(Debug, thiserror::Error)]
pub enum AnchorError {
    /// The OTS client (mock / CLI / network) failed to stamp.
    #[error("ots upstream failed: {0}")]
    Upstream(String),
    /// The returned receipt did not bind to the expected daily hash (anchor error guard).
    #[error("ots receipt does not bind to the expected daily hash")]
    BadBinding,
    /// Persisting the receipt failed.
    #[error("audit anchor db error: {0}")]
    Db(#[from] sqlx::Error),
}

/// OTS client error surfaced from [`crate::ots::OtsClient`].
#[derive(Debug, thiserror::Error)]
pub enum OtsError {
    /// Stamp request failed (network / CLI / mock).
    #[error("ots stamp failed: {0}")]
    Stamp(String),
    /// Receipt failed binding verification against the supplied hash.
    #[error("ots receipt binding mismatch")]
    Binding,
    /// The receipt bytes were not a parseable OTS envelope.
    #[error("ots receipt malformed: {0}")]
    Malformed(String),
}

/// Top-level audit error (§4.7).
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// SQLite error from the audit pool.
    #[error("audit db error: {0}")]
    Db(#[from] sqlx::Error),
    /// Segment cipher failure.
    #[error(transparent)]
    Cipher(#[from] CipherError),
    /// `what` JSON failed its `why`-bound schema validation (§4.4).
    #[error("audit `what` failed schema validation for {why}: {detail}")]
    SchemaValidation { why: &'static str, detail: String },
    /// JSON (de)serialization of who / what failed.
    #[error("audit json codec: {0}")]
    Json(#[from] serde_json::Error),
    /// The chain failed verification on startup; writes are refused (C-C-15).
    #[error("audit hash chain broken at seq {0:?}")]
    ChainBroken(Option<i64>),
    /// Daily-hash failure surfaced through the top-level API.
    #[error(transparent)]
    DailyHash(#[from] DailyHashError),
    /// Anchor failure surfaced through the top-level API.
    #[error(transparent)]
    Anchor(#[from] AnchorError),
}

impl AuditError {
    /// Map to the data-model API error code for the IPC boundary (backend/01 §1.12).
    #[must_use]
    pub fn error_code(&self) -> ErrorCode {
        match self {
            AuditError::ChainBroken(_) => ErrorCode::AuditChainBroken,
            AuditError::Anchor(_) => ErrorCode::OtsUpstream,
            AuditError::SchemaValidation { .. } => ErrorCode::BadRequest,
            _ => ErrorCode::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_broken_maps_to_audit_chain_broken() {
        let e = AuditError::ChainBroken(Some(3));
        assert_eq!(e.error_code(), ErrorCode::AuditChainBroken);
        assert_eq!(e.error_code().http_status(), 500);
    }

    #[test]
    fn anchor_failure_maps_to_ots_upstream() {
        let e = AuditError::Anchor(AnchorError::Upstream("calendar 502".into()));
        assert_eq!(e.error_code(), ErrorCode::OtsUpstream);
        assert_eq!(e.error_code().http_status(), 502);
    }

    #[test]
    fn schema_validation_maps_to_bad_request() {
        let e = AuditError::SchemaValidation {
            why: "case_freeze",
            detail: "missing case_id".into(),
        };
        assert_eq!(e.error_code(), ErrorCode::BadRequest);
    }
}
