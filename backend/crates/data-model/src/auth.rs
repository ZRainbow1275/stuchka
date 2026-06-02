//! `AuthLink` — one hop of the M15 group-case authorization chain (api §1.4.1, backend/01 §1.4.1).
//!
//! Cross-crate reconciliation B-7 places the canonical `AuthLink` type in `data-model` (the shared
//! domain crate) so `crates/sync` (patch / chain verification) and `crates/api` (the fact + import
//! patch bodies) share one definition rather than each redefining it. `crates/sync` re-exports this
//! type for back-compat and owns the verification logic (`verify_chain`); this module owns the data
//! shape plus the canonical signing payload (pure formatting, no signature deps).
//!
//! Time is `chrono` (W2), overriding the spec's `jiff::Timestamp`; `signed_at` is the shared
//! [`crate::time::Timestamp`] alias.

use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

/// One hop of the M15 authorization chain (api §1.4.1). Signed by `from_user` so the next holder
/// can prove the delegation. `scope` is the delegated patch scope (`facts_only` etc.).
///
/// Serialised `camelCase` so it travels unchanged inside the `.stuchka-patch` header, the
/// `ImportPatchReq` body, and `CreateFactReq.authorizationChain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthLink {
    /// Delegating peer id (ed25519 public key, base58).
    pub from_user: String,
    /// Delegated-to peer id (ed25519 public key, base58).
    pub to_user: String,
    /// Delegated scope string (matches `sync::PatchScope` serde values).
    pub scope: String,
    /// When the delegation was signed (chrono UTC; W2).
    pub signed_at: Timestamp,
    /// ed25519 signature (base64) over `from_user|to_user|scope|signed_at(rfc3339)` by `from_user`.
    pub signature: String,
}

impl AuthLink {
    /// The canonical bytes a link signature covers: `from_user|to_user|scope|signed_at(rfc3339)`.
    #[must_use]
    pub fn signing_payload(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}",
            self.from_user,
            self.to_user,
            self.scope,
            self.signed_at.to_rfc3339()
        )
        .into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn signing_payload_is_pipe_joined_rfc3339() {
        let link = AuthLink {
            from_user: "A".into(),
            to_user: "B".into(),
            scope: "full".into(),
            signed_at: chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            signature: "sig".into(),
        };
        let payload = String::from_utf8(link.signing_payload()).unwrap();
        assert!(
            payload.starts_with("A|B|full|2026-01-02T03:04:05"),
            "got {payload}"
        );
    }

    #[test]
    fn camel_case_serde_roundtrip() {
        let link = AuthLink {
            from_user: "from".into(),
            to_user: "to".into(),
            scope: "facts_only".into(),
            signed_at: crate::time::now(),
            signature: "s".into(),
        };
        let j = serde_json::to_string(&link).unwrap();
        assert!(j.contains("\"fromUser\""), "expected camelCase: {j}");
        let back: AuthLink = serde_json::from_str(&j).unwrap();
        assert_eq!(back, link);
    }
}
