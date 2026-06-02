//! KB impact notice — generated + persisted when the active KB version changes (data/03 §3.7).
//!
//! When `is_active` flips to a new KB version, an [`ImpactNotice`] is generated for every
//! *non-frozen* case whose referenced law-refs changed. Frozen cases are NEVER auto-rewritten
//! (INV-04). R1a delivers generation + persistence (schema + backend interface); the UI card is
//! R1b and the visual diff is R1.5.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An impact notice raised against a non-frozen case when the KB version changes (data/03 §3.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactNotice {
    pub id: Uuid,
    pub case_id: Uuid,
    /// D8 URN stable_ids whose body / version changed between `old_version` and `new_version`.
    pub affected_law_refs: Vec<String>,
    pub old_version: String,
    pub new_version: String,
    /// Natural-language summary (populated by M1 AI in R1b; R1a stores whatever the caller passes).
    pub diff_summary: String,
    pub created_at: DateTime<Utc>,
    pub dismissed: bool,
}

impl ImpactNotice {
    /// Build a fresh, undismissed impact notice (id = new v7 UUID, created_at = now).
    pub fn new(
        case_id: Uuid,
        affected_law_refs: Vec<String>,
        old_version: impl Into<String>,
        new_version: impl Into<String>,
        diff_summary: impl Into<String>,
    ) -> Self {
        Self {
            id: data_model::new_id(),
            case_id,
            affected_law_refs,
            old_version: old_version.into(),
            new_version: new_version.into(),
            diff_summary: diff_summary.into(),
            created_at: Utc::now(),
            dismissed: false,
        }
    }
}

/// Compute the set of law-ref stable_ids affected by a version change: every URN present in both
/// snapshots whose content_hash differs, plus every URN added or removed. The two inputs map
/// `stable_id -> content_hash` for the old and the new KB version respectively (data/03 §3.7).
///
/// This drives `ImpactNotice.affected_law_refs`. Only the *changed* refs a case actually uses are
/// later intersected by the caller; this helper returns the full changed set for the version pair.
pub fn changed_law_refs(
    old_hashes: &std::collections::BTreeMap<String, String>,
    new_hashes: &std::collections::BTreeMap<String, String>,
) -> Vec<String> {
    let mut changed = Vec::new();
    for (urn, new_hash) in new_hashes {
        match old_hashes.get(urn) {
            Some(old_hash) if old_hash == new_hash => {}
            _ => changed.push(urn.clone()),
        }
    }
    for urn in old_hashes.keys() {
        if !new_hashes.contains_key(urn) {
            changed.push(urn.clone());
        }
    }
    changed.sort();
    changed.dedup();
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn impact_notice_new_is_undismissed_with_v7_id() {
        let case_id = data_model::new_id();
        let n = ImpactNotice::new(
            case_id,
            vec!["law:某法/v2020-01-01/§1".to_string()],
            "2026-05-01-r1",
            "2026-06-01-r1",
            "第1条正文修订",
        );
        assert!(!n.dismissed);
        assert_eq!(n.case_id, case_id);
        assert_eq!(n.id.get_version_num(), 7);
    }

    #[test]
    fn impact_notice_camel_case_serde() {
        let n = ImpactNotice::new(data_model::new_id(), vec![], "a", "b", "summary");
        let j = serde_json::to_string(&n).unwrap();
        assert!(j.contains("\"affectedLawRefs\""));
        assert!(j.contains("\"oldVersion\""));
        let back: ImpactNotice = serde_json::from_str(&j).unwrap();
        assert_eq!(back, n);
    }

    #[test]
    fn changed_law_refs_detects_modified_added_removed() {
        let mut old = BTreeMap::new();
        old.insert("law:A/v2020-01-01/§1".to_string(), "h1".to_string());
        old.insert("law:B/v2020-01-01/§1".to_string(), "h2".to_string()); // removed
        old.insert("law:C/v2020-01-01/§1".to_string(), "h3".to_string()); // unchanged

        let mut new = BTreeMap::new();
        new.insert("law:A/v2020-01-01/§1".to_string(), "h1_changed".to_string()); // modified
        new.insert("law:C/v2020-01-01/§1".to_string(), "h3".to_string());
        new.insert("law:D/v2020-01-01/§1".to_string(), "h4".to_string()); // added

        let changed = changed_law_refs(&old, &new);
        assert!(changed.contains(&"law:A/v2020-01-01/§1".to_string()));
        assert!(changed.contains(&"law:B/v2020-01-01/§1".to_string()));
        assert!(changed.contains(&"law:D/v2020-01-01/§1".to_string()));
        assert!(!changed.contains(&"law:C/v2020-01-01/§1".to_string()));
        assert_eq!(changed.len(), 3);
    }
}
