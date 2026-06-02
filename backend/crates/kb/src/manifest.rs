//! KB global manifest + deterministic `global_hash` aggregate (data/03 §3.1, deploy/04 §4.2.2).
//!
//! `manifest.global_hash` is the INV-04 freeze anchor: it equals `case.kb_version_hash` at the
//! moment a case freezes (KBC-02). It is the SHA-256 of the *sorted* `file_path + per-file
//! SHA-256` lines, exactly matching the publishing recipe in deploy/04 §4.2.2:
//!
//! ```text
//! kb_hash = sha256( sorted( file_path + sha256(file_content) for file in content/ ) )
//! ```
//!
//! Because the inputs are sorted lexicographically before aggregation, the hash is deterministic
//! and reproducible across platforms regardless of directory-walk order.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::KbError;

/// The KB total category / subcategory counts (data/01 §1.7, KBC-03). Constant invariants.
pub const CATEGORY_TOTAL: i32 = 20;
pub const SUBCATEGORY_TOTAL: i32 = 85;

/// Global KB manifest (`manifest.json`, data/03 §3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KbManifest {
    pub version_label: String,
    pub generated_at: DateTime<Utc>,
    /// Deterministic aggregate hash of all content files (INV-04 freeze anchor).
    pub global_hash: String,
    pub schema_version: i32,
    pub file_count: i32,
    pub law_count_national: i32,
    pub law_count_local: i32,
    /// MUST be 20 (KBC-03).
    pub category_total: i32,
    /// MUST be 85 (KBC-03).
    pub subcategory_total: i32,
}

impl KbManifest {
    /// Validate the manifest's internal invariants (KBC-02 / KBC-03):
    /// - `category_total == 20` and `subcategory_total == 85`;
    /// - `global_hash` is 64 lower-case hex characters.
    pub fn validate(&self) -> Result<(), KbError> {
        if self.category_total != CATEGORY_TOTAL || self.subcategory_total != SUBCATEGORY_TOTAL {
            return Err(KbError::CategoryCountInvalid {
                categories: self.category_total.max(0) as usize,
                subcategories: self.subcategory_total.max(0) as usize,
            });
        }
        if !is_sha256_hex(&self.global_hash) {
            return Err(KbError::GlobalHashMismatch {
                expected: "<64-hex>".to_string(),
                actual: self.global_hash.clone(),
            });
        }
        Ok(())
    }

    /// Verify the manifest's `global_hash` matches a freshly recomputed aggregate over the given
    /// `(file_path, file_content_sha256)` entries (KBC-02). Returns
    /// [`KbError::GlobalHashMismatch`] on divergence.
    pub fn verify_global_hash<I, P, H>(&self, file_hashes: I) -> Result<(), KbError>
    where
        I: IntoIterator<Item = (P, H)>,
        P: AsRef<str>,
        H: AsRef<str>,
    {
        let recomputed = compute_global_hash(file_hashes);
        if recomputed != self.global_hash {
            return Err(KbError::GlobalHashMismatch {
                expected: self.global_hash.clone(),
                actual: recomputed,
            });
        }
        Ok(())
    }
}

/// Compute the deterministic aggregate `global_hash` from `(file_path, file_content_sha256)`
/// pairs (deploy/04 §4.2.2 recipe). The pairs are sorted by path before hashing so the result is
/// independent of iteration order; each contributes one `"<path>\u{1F}<file_hash>\n"` line.
pub fn compute_global_hash<I, P, H>(file_hashes: I) -> String
where
    I: IntoIterator<Item = (P, H)>,
    P: AsRef<str>,
    H: AsRef<str>,
{
    let mut lines: Vec<String> = file_hashes
        .into_iter()
        .map(|(p, h)| format!("{}\u{1F}{}", p.as_ref(), h.as_ref()))
        .collect();
    lines.sort();
    let mut hasher = Sha256::new();
    for line in &lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// SHA-256 hex of a file's raw bytes (the per-file leaf hash that feeds the aggregate / the
/// `checksums.txt` entry, KBC-04).
pub fn file_content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// True when `s` is exactly 64 lower-case hex characters.
fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_manifest(global_hash: &str) -> KbManifest {
        KbManifest {
            version_label: "2026-05-12-r1".to_string(),
            generated_at: Utc.with_ymd_and_hms(2026, 5, 12, 8, 0, 0).unwrap(),
            global_hash: global_hash.to_string(),
            schema_version: 1,
            file_count: 247,
            law_count_national: 32,
            law_count_local: 124,
            category_total: CATEGORY_TOTAL,
            subcategory_total: SUBCATEGORY_TOTAL,
        }
    }

    /// KBC-02: global_hash is a deterministic aggregate independent of input order.
    #[test]
    fn kbc02_global_hash_is_order_independent() {
        let forward = compute_global_hash([
            ("laws/a.json", file_content_hash(b"AAA")),
            ("laws/b.json", file_content_hash(b"BBB")),
            ("categories.yaml", file_content_hash(b"CCC")),
        ]);
        let shuffled = compute_global_hash([
            ("categories.yaml", file_content_hash(b"CCC")),
            ("laws/b.json", file_content_hash(b"BBB")),
            ("laws/a.json", file_content_hash(b"AAA")),
        ]);
        assert_eq!(forward, shuffled, "aggregate must be order-independent");
        assert_eq!(forward.len(), 64);
    }

    /// KBC-02: any per-file content change flips the aggregate.
    #[test]
    fn kbc02_global_hash_changes_on_content_change() {
        let base = compute_global_hash([("laws/a.json", file_content_hash(b"AAA"))]);
        let changed = compute_global_hash([("laws/a.json", file_content_hash(b"AAB"))]);
        assert_ne!(base, changed);
    }

    /// KBC-02: the manifest's stored global_hash is the same aggregate the client recomputes,
    /// so it can serve as the `case.kb_version_hash` freeze anchor (INV-04).
    #[test]
    fn kbc02_manifest_verifies_against_recomputed_aggregate() {
        let files = [
            (
                "laws/national/labor-contract-law/v2012-12-28.json",
                file_content_hash(b"body1"),
            ),
            ("categories.yaml", file_content_hash(b"cats")),
        ];
        let gh = compute_global_hash(files.clone());
        let m = sample_manifest(&gh);
        m.validate().unwrap();
        m.verify_global_hash(files).unwrap();
    }

    #[test]
    fn verify_global_hash_detects_mismatch() {
        let m = sample_manifest(&"a".repeat(64));
        let err = m
            .verify_global_hash([("laws/a.json", file_content_hash(b"AAA"))])
            .unwrap_err();
        assert_eq!(err.code(), "E_KB_CHECKSUM_MISMATCH");
    }

    #[test]
    fn validate_rejects_wrong_category_totals() {
        let mut m = sample_manifest(&"a".repeat(64));
        m.subcategory_total = 84;
        let err = m.validate().unwrap_err();
        assert!(matches!(
            err,
            KbError::CategoryCountInvalid {
                categories: 20,
                subcategories: 84
            }
        ));
    }

    #[test]
    fn validate_rejects_bad_global_hash() {
        let m = sample_manifest("not-a-hash");
        assert!(m.validate().is_err());
    }

    #[test]
    fn manifest_json_roundtrips() {
        let m = sample_manifest(&"f".repeat(64));
        let j = serde_json::to_string(&m).unwrap();
        let back: KbManifest = serde_json::from_str(&j).unwrap();
        assert_eq!(m, back);
    }
}
