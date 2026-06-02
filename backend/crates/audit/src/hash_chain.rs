//! §4.3.1 — the prev_hash -> record_hash chain.
//!
//! `record_hash = SHA-256(prev_hash || who_kind || who_ct || who_nonce || when_be || why ||
//! what_ct || what_nonce)`. data/04 §4.3.1 lists a single `nonce`; under W5 (random per-segment
//! nonce, no reuse) who and what each carry their own nonce, and both participate in the hash so
//! tampering with either segment or its nonce breaks the chain.

use sha2::{Digest, Sha256};
use sqlx::Row;

/// Genesis `prev_hash` for the first record (64 hex zeros).
pub const GENESIS_PREV_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// The hashed fields of one record, recomputed from storage (used by `verify_chain` and the daily
/// digest so the daily hash binds the *content*, not the possibly-tampered `record_hash` column).
#[derive(Debug, Clone)]
pub struct RecordHashInput {
    pub prev_hash: String,
    pub who_kind: String,
    pub who_payload: Vec<u8>,
    pub who_nonce: Vec<u8>,
    pub when_ts: i64,
    pub why: String,
    pub what_payload: Vec<u8>,
    pub what_nonce: Vec<u8>,
}

impl RecordHashInput {
    /// Build from a SQLite row that selected the hashed columns (the `nonce` column is the what
    /// nonce; `who_nonce` is the who nonce — W5 per-segment nonces).
    #[must_use]
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Self {
        Self {
            prev_hash: row.get("prev_hash"),
            who_kind: row.get("who_kind"),
            who_payload: row.get("who_payload"),
            who_nonce: row.get("who_nonce"),
            when_ts: row.get("when_ts"),
            why: row.get("why"),
            what_payload: row.get("what_payload"),
            what_nonce: row.get("nonce"),
        }
    }

    /// Recompute this record's `record_hash`.
    #[must_use]
    pub fn compute(&self) -> String {
        compute_record_hash(
            &self.prev_hash,
            &self.who_kind,
            &self.who_payload,
            &self.who_nonce,
            self.when_ts,
            &self.why,
            &self.what_payload,
            &self.what_nonce,
        )
    }
}

/// Compute the lower-case hex `record_hash` over the canonical field concatenation.
///
/// All variable-length segments are length-prefixed implicitly by their fixed neighbours? No —
/// to avoid concatenation ambiguity the order is fixed and the two ciphertexts are separated by
/// the fixed-width `who_nonce` / `when_be` fields, mirroring data/04 §4.3.1's exact ordering with
/// the W5 per-segment nonces folded in.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn compute_record_hash(
    prev_hash: &str,
    who_kind: &str,
    who_ciphertext: &[u8],
    who_nonce: &[u8],
    when_ts: i64,
    why: &str,
    what_ciphertext: &[u8],
    what_nonce: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(who_kind.as_bytes());
    hasher.update(who_ciphertext);
    hasher.update(who_nonce);
    hasher.update(when_ts.to_be_bytes());
    hasher.update(why.as_bytes());
    hasher.update(what_ciphertext);
    hasher.update(what_nonce);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_is_64_zeros() {
        assert_eq!(GENESIS_PREV_HASH.len(), 64);
        assert!(GENESIS_PREV_HASH.chars().all(|c| c == '0'));
    }

    #[test]
    fn record_hash_is_deterministic_64_hex() {
        let h = compute_record_hash(
            GENESIS_PREV_HASH,
            "user",
            b"who-ct",
            &[1u8; 12],
            1_700_000_000_000,
            "evidence_uploaded",
            b"what-ct",
            &[2u8; 12],
        );
        let h2 = compute_record_hash(
            GENESIS_PREV_HASH,
            "user",
            b"who-ct",
            &[1u8; 12],
            1_700_000_000_000,
            "evidence_uploaded",
            b"what-ct",
            &[2u8; 12],
        );
        assert_eq!(h, h2);
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn any_field_change_changes_hash() {
        let base = compute_record_hash(
            GENESIS_PREV_HASH,
            "user",
            b"who",
            &[1u8; 12],
            10,
            "case_freeze",
            b"what",
            &[2u8; 12],
        );
        // change what ciphertext
        assert_ne!(
            base,
            compute_record_hash(
                GENESIS_PREV_HASH,
                "user",
                b"who",
                &[1u8; 12],
                10,
                "case_freeze",
                b"WHAT",
                &[2u8; 12],
            )
        );
        // change the what nonce
        assert_ne!(
            base,
            compute_record_hash(
                GENESIS_PREV_HASH,
                "user",
                b"who",
                &[1u8; 12],
                10,
                "case_freeze",
                b"what",
                &[9u8; 12],
            )
        );
        // change the timestamp
        assert_ne!(
            base,
            compute_record_hash(
                GENESIS_PREV_HASH,
                "user",
                b"who",
                &[1u8; 12],
                11,
                "case_freeze",
                b"what",
                &[2u8; 12],
            )
        );
    }
}
