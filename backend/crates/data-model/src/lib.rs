//! `data-model` — first-class objects + state machines + error codes (D9).
//!
//! Workspace leaf domain crate (master-index §0.4 / D6; `crates/types` merged in).
//! Time = chrono, money = `rust_decimal::Decimal`, ids = UUID v7. The `sqlx` feature
//! (off by default) gates `FromRow`/`Type` derives so the db layer can persist these
//! types while rule-engine never links sqlx (INV-01 boundary).

pub mod auth;
pub mod case;
pub mod claim;
pub mod enums;
pub mod error;
pub mod evidence;
pub mod fact;
pub mod group;
pub mod id;
pub mod law_ref;
pub mod pii;
pub mod sensitivity;
pub mod state_machine;
pub mod time;
pub mod traits;
pub mod user_setting;

// Re-exports for ergonomic downstream use.
pub use auth::AuthLink;
pub use case::Case;
pub use claim::Claim;
pub use enums::{
    AuditCategory, CaseStatus, ClaimStatus, ClaimType, CoverageTag, CoverageTier, DisputeSubtype,
    EvidenceCategory, EvidenceStatus, FactCategory, FactSource, FactStatus, IdentityType, LawLevel,
    NextAction, SourceTag,
};
pub use error::{ApiEnvelope, ApiError, ErrorCode, ALL_ERROR_CODES};
pub use evidence::Evidence;
pub use fact::CaseFact;
pub use group::{Group, GroupContributor};
pub use id::new_id;
pub use law_ref::{
    content_hash, normalize_punct, parse as parse_law_ref, to_stable_id as law_ref_to_stable_id,
    ItemBracket, LawRef, LawRefError, LawRefParts,
};
pub use pii::{PiiHit, PiiKind, PiiLayer, RouteHint, Span};
pub use sensitivity::{grade_of, DataGrade, SENSITIVE_FIELD_REGISTRY};
pub use state_machine::{
    validate_case_transition, validate_claim_transition, validate_evidence_transition,
    validate_fact_transition, StateTransitionError,
};
pub use time::{now, PlainDate, Timestamp};
pub use traits::{AuditSubject, Auditable, Identified, Versioned};
pub use user_setting::{FsEncryptionStatus, RecoveryMethod, UserSetting};

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "data-model";

/// Schema version reported by [`Versioned::schema_version`].
pub const SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "data-model");
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(super::SCHEMA_VERSION, 1);
    }
}
