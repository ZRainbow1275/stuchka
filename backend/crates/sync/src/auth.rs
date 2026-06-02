//! Authorization-chain verification (03-sync-yjs §3.7.3) + ed25519 patch-header signature check.
//!
//! `AuthLink` is the M15 group-case authorization link (api §1.4.1). Per cross-crate reconciliation
//! B-7 the canonical type now lives in `data-model` (shared between sync + api); this module
//! re-exports it for back-compat and owns the *verification logic* (`verify_chain`). It is
//! `Serialize` + `Deserialize` so it travels inside the `.stuchka-patch` header and the
//! `ImportPatchReq` body unchanged. Time is `chrono` (W2), overriding the spec's `jiff::Timestamp`.
//!
//! ed25519-dalek 2.1 is the only signature scheme (§E / resolves I-6: minisign is reserved for
//! upgrade packages, not patches). `verify_strict` rejects malleable signatures.

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, VerifyingKey};

use crate::patch::{PatchHeader, PatchScope};

/// The canonical authorization-chain link (cross-crate reconciliation B-7): defined in `data-model`
/// and re-exported here so existing `sync::AuthLink` paths keep working.
pub use data_model::AuthLink;

/// Authorization-link validity window (03-sync-yjs §3.7.3: 30-day time window per hop).
pub const LINK_VALIDITY_DAYS: i64 = 30;

/// Authorization-chain failures (03-sync-yjs §3.7.3 -> `E_PATCH_AUTHORIZATION`).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    /// Chain head is not the target-case owner, or the chain does not reach the contributor.
    #[error("authorization chain incomplete")]
    ChainIncomplete,
    /// A hop's signature did not verify against `from_user`'s public key.
    #[error("link signature invalid at hop {0}")]
    LinkSignatureInvalid(usize),
    /// A hop is older than the 30-day window.
    #[error("link expired at hop {0}")]
    LinkExpired(usize),
    /// The leaf hop's delegated scope does not cover the requested patch scope.
    #[error("requested scope not covered by the chain")]
    ScopeNotCovered,
    /// A `from_user` / `to_user` peer id was not a valid base58 ed25519 key.
    #[error("malformed peer key at hop {0}")]
    MalformedKey(usize),
}

/// Decode a base58 ed25519 public key (peer id) into a verifying key.
fn decode_pubkey(peer_id: &str) -> Option<VerifyingKey> {
    let bytes = bs58::decode(peer_id).into_vec().ok()?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

/// Decode a base64 ed25519 signature.
fn decode_signature(b64: &str) -> Option<Signature> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    Signature::from_slice(&bytes).ok()
}

/// Does a granted scope cover the requested scope? `full` covers everything; `facts_and_evidences`
/// covers `facts_only`; `facts_only` only covers itself.
fn scope_covers(granted: PatchScope, requested: PatchScope) -> bool {
    use PatchScope::*;
    matches!(
        (granted, requested),
        (Full, _)
            | (FactsAndEvidences, FactsAndEvidences)
            | (FactsAndEvidences, FactsOnly)
            | (FactsOnly, FactsOnly)
    )
}

/// Verify the authorization chain for an incoming patch (03-sync-yjs §3.7.3).
///
/// Rules: the head hop's `from_user` must be `target_case_owner`; hops must be contiguous
/// (`hop[i].to_user == hop[i+1].from_user`); the final hop's `to_user` must be `contributor_id`;
/// every hop verifies under `from_user`'s key and is within the 30-day window; and the final hop's
/// delegated scope must cover the requested `scope`. Empty chain => `ChainIncomplete`.
pub fn verify_chain(
    chain: &[AuthLink],
    target_case_owner: &str,
    contributor_id: &str,
    scope: PatchScope,
    now: DateTime<Utc>,
) -> Result<(), AuthError> {
    let Some(head) = chain.first() else {
        return Err(AuthError::ChainIncomplete);
    };
    if head.from_user != target_case_owner {
        return Err(AuthError::ChainIncomplete);
    }
    let last = chain.last().expect("non-empty checked above");
    if last.to_user != contributor_id {
        return Err(AuthError::ChainIncomplete);
    }

    let cutoff = now - Duration::days(LINK_VALIDITY_DAYS);
    let mut expected_from = target_case_owner.to_string();
    for (i, link) in chain.iter().enumerate() {
        // Contiguity: each hop must start where the previous one ended.
        if link.from_user != expected_from {
            return Err(AuthError::ChainIncomplete);
        }
        // Time window.
        if link.signed_at < cutoff {
            return Err(AuthError::LinkExpired(i));
        }
        // Signature by `from_user`.
        let key = decode_pubkey(&link.from_user).ok_or(AuthError::MalformedKey(i))?;
        let sig = decode_signature(&link.signature).ok_or(AuthError::LinkSignatureInvalid(i))?;
        key.verify_strict(&link.signing_payload(), &sig)
            .map_err(|_| AuthError::LinkSignatureInvalid(i))?;
        expected_from = link.to_user.clone();
    }

    // Leaf scope must cover what is being imported.
    let granted: PatchScope = last.scope.parse().map_err(|_| AuthError::ScopeNotCovered)?;
    if !scope_covers(granted, scope) {
        return Err(AuthError::ScopeNotCovered);
    }
    Ok(())
}

/// Verify a `.stuchka-patch` header signature against the contributor's public key (§3.7.3 step 2).
///
/// The header signs `case_id|contributor_id|payload_sha256` (header.signed_fields documents the
/// layout). Failure -> the caller maps to `E_PATCH_INVALID`.
pub fn verify_patch_signature(
    header: &PatchHeader,
    payload: &[u8],
    contributor_pubkey_base58: &str,
) -> Result<(), AuthError> {
    use sha2::{Digest, Sha256};
    let payload_sha256 = {
        let mut h = Sha256::new();
        h.update(payload);
        let d = h.finalize();
        d.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    let signed = format!(
        "{}|{}|{}",
        header.case_id, header.contributor_id, payload_sha256
    );
    let key = decode_pubkey(contributor_pubkey_base58).ok_or(AuthError::MalformedKey(0))?;
    let sig = decode_signature(&header.signature).ok_or(AuthError::LinkSignatureInvalid(0))?;
    key.verify_strict(signed.as_bytes(), &sig)
        .map_err(|_| AuthError::LinkSignatureInvalid(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn keypair(seed: u8) -> (SigningKey, String) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let peer_id = bs58::encode(sk.verifying_key().to_bytes()).into_string();
        (sk, peer_id)
    }

    fn sign_link(
        sk: &SigningKey,
        from: &str,
        to: &str,
        scope: &str,
        at: DateTime<Utc>,
    ) -> AuthLink {
        use base64::Engine;
        let mut link = AuthLink {
            from_user: from.to_string(),
            to_user: to.to_string(),
            scope: scope.to_string(),
            signed_at: at,
            signature: String::new(),
        };
        let sig = sk.sign(&link.signing_payload());
        link.signature = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
        link
    }

    #[test]
    fn single_hop_chain_verifies() {
        let now = Utc::now();
        let (owner_sk, owner) = keypair(1);
        let (_c_sk, contributor) = keypair(2);
        let chain = vec![sign_link(&owner_sk, &owner, &contributor, "full", now)];
        assert!(verify_chain(&chain, &owner, &contributor, PatchScope::FactsOnly, now).is_ok());
    }

    #[test]
    fn head_not_owner_is_incomplete() {
        let now = Utc::now();
        let (owner_sk, owner) = keypair(1);
        let (_c_sk, contributor) = keypair(2);
        let chain = vec![sign_link(&owner_sk, &owner, &contributor, "full", now)];
        let err = verify_chain(
            &chain,
            "someone-else",
            &contributor,
            PatchScope::FactsOnly,
            now,
        )
        .unwrap_err();
        assert_eq!(err, AuthError::ChainIncomplete);
    }

    #[test]
    fn expired_hop_is_rejected() {
        let now = Utc::now();
        let old = now - Duration::days(31);
        let (owner_sk, owner) = keypair(1);
        let (_c_sk, contributor) = keypair(2);
        let chain = vec![sign_link(&owner_sk, &owner, &contributor, "full", old)];
        let err =
            verify_chain(&chain, &owner, &contributor, PatchScope::FactsOnly, now).unwrap_err();
        assert_eq!(err, AuthError::LinkExpired(0));
    }

    #[test]
    fn scope_not_covered_is_rejected() {
        let now = Utc::now();
        let (owner_sk, owner) = keypair(1);
        let (_c_sk, contributor) = keypair(2);
        // granted facts_only but requesting full
        let chain = vec![sign_link(
            &owner_sk,
            &owner,
            &contributor,
            "facts_only",
            now,
        )];
        let err = verify_chain(&chain, &owner, &contributor, PatchScope::Full, now).unwrap_err();
        assert_eq!(err, AuthError::ScopeNotCovered);
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let now = Utc::now();
        let (owner_sk, owner) = keypair(1);
        let (_c_sk, contributor) = keypair(2);
        let mut chain = vec![sign_link(&owner_sk, &owner, &contributor, "full", now)];
        chain[0].scope = "facts_only".into(); // signed over "full" -> signature now invalid
        let err =
            verify_chain(&chain, &owner, &contributor, PatchScope::FactsOnly, now).unwrap_err();
        assert_eq!(err, AuthError::LinkSignatureInvalid(0));
    }

    #[test]
    fn two_hop_chain_must_be_contiguous() {
        let now = Utc::now();
        let (owner_sk, owner) = keypair(1);
        let (mid_sk, mid) = keypair(2);
        let (_leaf_sk, leaf) = keypair(3);
        let chain = vec![
            sign_link(&owner_sk, &owner, &mid, "full", now),
            sign_link(&mid_sk, &mid, &leaf, "full", now),
        ];
        assert!(verify_chain(&chain, &owner, &leaf, PatchScope::Full, now).is_ok());

        // Break contiguity: second hop starts from someone else.
        let (other_sk, other) = keypair(9);
        let bad = vec![
            sign_link(&owner_sk, &owner, &mid, "full", now),
            sign_link(&other_sk, &other, &leaf, "full", now),
        ];
        assert_eq!(
            verify_chain(&bad, &owner, &leaf, PatchScope::Full, now).unwrap_err(),
            AuthError::ChainIncomplete
        );
    }
}
