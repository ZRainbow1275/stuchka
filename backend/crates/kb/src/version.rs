//! KB version record + activation (data/03 §3.4; backend/00 §0.4 `version.rs`).
//!
//! Mirrors the `kb_version` table (data/03 §3.4 is authoritative over backend/02 §2.8 per the
//! cross-crate reconciliation: `version_label` PK + `global_hash` + `manifest_blob`). The DB layer
//! owns the migration; this module models the row and the activation invariant in-memory so the
//! api / db layers share one shape.
//!
//! Activation invariant (data/03 §3.4): at most one version may be `is_active`. [`activate`]
//! enforces it over an in-memory set so callers can validate before writing the partial unique
//! index `idx_kb_active_singleton`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::manifest::KbManifest;

/// A row of the `kb_version` table (data/03 §3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct KbVersion {
    /// `version_label` PRIMARY KEY (e.g. `2026-05-12-r1`).
    pub version_label: String,
    /// `global_hash` UNIQUE — the INV-04 freeze anchor (== `case.kb_version_hash`).
    pub global_hash: String,
    pub generated_at: DateTime<Utc>,
    pub installed_at: DateTime<Utc>,
    pub schema_version: i32,
    pub file_count: i32,
    /// Whether this is the currently-active version (singleton: at most one TRUE).
    pub is_active: bool,
}

impl KbVersion {
    /// Build a (non-active) version record from a validated manifest (data/03 §3.4). The manifest
    /// is the source of `version_label` / `global_hash` / `generated_at` (INV-04 anchor).
    pub fn from_manifest(manifest: &KbManifest, installed_at: DateTime<Utc>) -> Self {
        Self {
            version_label: manifest.version_label.clone(),
            global_hash: manifest.global_hash.clone(),
            generated_at: manifest.generated_at,
            installed_at,
            schema_version: manifest.schema_version,
            file_count: manifest.file_count,
            is_active: false,
        }
    }
}

/// Activate `target_label` within `versions`, clearing `is_active` on every other row so the
/// singleton invariant (data/03 §3.4) holds. Returns the previously-active label (if any) so the
/// caller can generate impact notices for the version transition (data/03 §3.7).
pub fn activate(versions: &mut [KbVersion], target_label: &str) -> Option<String> {
    let mut previous = None;
    let mut found = false;
    for v in versions.iter_mut() {
        if v.is_active && v.version_label != target_label {
            previous = Some(v.version_label.clone());
        }
        if v.version_label == target_label {
            v.is_active = true;
            found = true;
        } else {
            v.is_active = false;
        }
    }
    debug_assert!(found, "activate target must exist in the version set");
    previous
}

/// True iff at most one version is active (the singleton invariant, data/03 §3.4).
pub fn active_singleton_holds(versions: &[KbVersion]) -> bool {
    versions.iter().filter(|v| v.is_active).count() <= 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{CATEGORY_TOTAL, SUBCATEGORY_TOTAL};
    use chrono::TimeZone;

    fn manifest(label: &str, hash: &str) -> KbManifest {
        KbManifest {
            version_label: label.to_string(),
            generated_at: Utc.with_ymd_and_hms(2026, 5, 12, 8, 0, 0).unwrap(),
            global_hash: hash.to_string(),
            schema_version: 1,
            file_count: 10,
            law_count_national: 5,
            law_count_local: 5,
            category_total: CATEGORY_TOTAL,
            subcategory_total: SUBCATEGORY_TOTAL,
        }
    }

    #[test]
    fn from_manifest_carries_global_hash_anchor() {
        let m = manifest("2026-05-12-r1", &"a".repeat(64));
        let v = KbVersion::from_manifest(&m, Utc::now());
        assert_eq!(v.version_label, "2026-05-12-r1");
        assert_eq!(v.global_hash, "a".repeat(64));
        assert!(
            !v.is_active,
            "freshly installed version is inactive until activated"
        );
    }

    #[test]
    fn activate_enforces_singleton_and_returns_previous() {
        let mut versions = vec![
            KbVersion::from_manifest(&manifest("v1", &"1".repeat(64)), Utc::now()),
            KbVersion::from_manifest(&manifest("v2", &"2".repeat(64)), Utc::now()),
        ];
        versions[0].is_active = true; // v1 currently active

        let prev = activate(&mut versions, "v2");
        assert_eq!(prev.as_deref(), Some("v1"));
        assert!(active_singleton_holds(&versions));
        assert!(!versions[0].is_active);
        assert!(versions[1].is_active);
    }

    #[test]
    fn activate_from_none_returns_none() {
        let mut versions = vec![KbVersion::from_manifest(
            &manifest("v1", &"1".repeat(64)),
            Utc::now(),
        )];
        let prev = activate(&mut versions, "v1");
        assert_eq!(prev, None);
        assert!(versions[0].is_active);
    }
}
