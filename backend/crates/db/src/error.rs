//! db-layer error type (maps onto data-model `ErrorCode` at the API boundary).
//!
//! `InvalidTransition` wraps the data-model state-machine guard so the application layer
//! returns `E_INVALID_TRANSITION` (backend/02 §2.8 INV-04 + data/01 state machines).

use data_model::state_machine::StateTransitionError;

/// Errors surfaced by the db access layer.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// Underlying sqlx failure (connection / SQL / decode).
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// State-machine guard rejected the transition (E_INVALID_TRANSITION).
    #[error(transparent)]
    InvalidTransition(#[from] StateTransitionError),

    /// INV-04: an UPDATE attempted to change a frozen `kb_version_hash` (E_INVALID_TRANSITION).
    #[error("INV-04 violation: kb_version_hash is frozen and cannot be modified")]
    KbVersionFrozen,

    /// Row not found where one was required (E_NOT_FOUND).
    #[error("record not found")]
    NotFound,

    /// JSON (de)serialization for a TEXT-as-JSON column failed.
    #[error("json codec error: {0}")]
    Json(#[from] serde_json::Error),

    /// Fixed-point decimal parse for a money TEXT column failed.
    #[error("decimal codec error: {0}")]
    Decimal(#[from] rust_decimal::Error),

    /// UUID parse for a TEXT-UUID column failed.
    #[error("uuid codec error: {0}")]
    Uuid(#[from] uuid::Error),
}

/// Convenience result alias for the db layer.
pub type DbResult<T> = Result<T, DbError>;
