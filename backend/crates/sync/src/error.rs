//! Crate-wide error type. Maps onto the `data-model` error-code table at the api boundary
//! (`E_PATCH_INVALID` / `E_PATCH_AUTHORIZATION` / `E_INTERNAL`) via [`SyncError::error_code`].

use data_model::ErrorCode;

/// Result alias for sync operations.
pub type SyncResult<T> = Result<T, SyncError>;

/// Errors raised by the sync engine. Each variant carries the [`ErrorCode`] the api layer should
/// surface (backend/01 §1.12) so the IPC envelope stays consistent.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// A Yjs update / state-vector failed to decode or apply (`E_INTERNAL`).
    #[error("yjs codec/apply error: {0}")]
    Yjs(String),

    /// `.stuchka-patch` magic / version / length / footer-hash check failed (`E_PATCH_INVALID`).
    #[error("patch is invalid: {0}")]
    PatchInvalid(String),

    /// ed25519 signature verification failed (`E_PATCH_INVALID`).
    #[error("signature verification failed: {0}")]
    SignatureInvalid(String),

    /// Authorization chain check failed (`E_PATCH_AUTHORIZATION`).
    #[error("authorization rejected: {0}")]
    Authorization(#[from] crate::auth::AuthError),

    /// SyncHello handshake rejected: bad signature, untrusted peer, or version mismatch.
    #[error("handshake rejected: {0}")]
    Handshake(String),

    /// A persistence (sqlx) operation failed (`E_INTERNAL`).
    #[error("persistence error: {0}")]
    Db(String),

    /// An audit write failed (`E_INTERNAL`).
    #[error("audit error: {0}")]
    Audit(String),

    /// mDNS daemon / discovery error (`E_INTERNAL`).
    #[error("mdns error: {0}")]
    Mdns(String),

    /// Serialization of a control frame / header failed (`E_INTERNAL`).
    #[error("serde error: {0}")]
    Serde(String),
}

impl SyncError {
    /// The api-layer error code to surface for this error (backend/01 §1.12).
    #[must_use]
    pub fn error_code(&self) -> ErrorCode {
        match self {
            SyncError::PatchInvalid(_) | SyncError::SignatureInvalid(_) => ErrorCode::PatchInvalid,
            SyncError::Authorization(_) => ErrorCode::PatchAuthorization,
            SyncError::Handshake(_) => ErrorCode::InvalidToken,
            SyncError::Yjs(_)
            | SyncError::Db(_)
            | SyncError::Audit(_)
            | SyncError::Mdns(_)
            | SyncError::Serde(_) => ErrorCode::Internal,
        }
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(e: serde_json::Error) -> Self {
        SyncError::Serde(e.to_string())
    }
}

impl From<sqlx::Error> for SyncError {
    fn from(e: sqlx::Error) -> Self {
        SyncError::Db(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_map_to_spec() {
        assert_eq!(
            SyncError::PatchInvalid("x".into()).error_code(),
            ErrorCode::PatchInvalid
        );
        assert_eq!(
            SyncError::SignatureInvalid("x".into()).error_code(),
            ErrorCode::PatchInvalid
        );
        assert_eq!(
            SyncError::Authorization(crate::auth::AuthError::ChainIncomplete).error_code(),
            ErrorCode::PatchAuthorization
        );
        assert_eq!(SyncError::Yjs("x".into()).error_code(), ErrorCode::Internal);
    }
}
