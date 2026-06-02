//! LawRef — law reference snapshot (data/01 §1.4) + D8 URN parser / content hash (data/02).
//!
//! The frozen snapshot struct [`LawRef`] lives here; the D8 URN grammar and the
//! content-addressing hash live in the [`parser`] and [`content_hash`] submodules. The parser /
//! hash were a MAJOR data-model gap (downstream KB / ai-dispatcher depend on them); they are now
//! fully implemented (no `unimplemented!`).

pub mod content_hash;
pub mod parser;

pub use content_hash::{content_hash, normalize_punct};
pub use parser::{parse, to_stable_id, ItemBracket, LawRefError, LawRefParts};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::{PlainDate, Timestamp};
use crate::traits::Identified;

/// Frozen snapshot of a law reference used by a case (data/01 §1.4.5). Ensures all
/// references are snapshotted together when a case freezes (INV-04).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct LawRef {
    pub id: Uuid,
    /// D8 URN, e.g. `law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1` (data/02). Parse with
    /// [`parser::parse`].
    pub stable_id: String,
    /// Normalized-text SHA-256 hex 64 (local content addressing, data/02 §2.3). Compute with
    /// [`content_hash::content_hash`].
    pub content_hash: String,
    pub kb_version_label: String,
    pub title: String,
    pub version_date: PlainDate,
    pub article: String,
    pub body_snapshot: String,
    pub created_at: Timestamp,
}

impl Identified for LawRef {
    fn id(&self) -> Uuid {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::new_id;
    use crate::time::now;

    #[test]
    fn law_ref_serde_roundtrip_preserves_urn() {
        let lr = LawRef {
            id: new_id(),
            stable_id: "law:中华人民共和国劳动合同法/v2012-12-28/§39/¶1".to_string(),
            content_hash: "b".repeat(64),
            kb_version_label: "2026-05-12-r1".to_string(),
            title: "中华人民共和国劳动合同法".to_string(),
            version_date: PlainDate::from_ymd_opt(2012, 12, 28).unwrap(),
            article: "§39/¶1".to_string(),
            body_snapshot: "劳动者有下列情形之一的，用人单位可以解除劳动合同……".to_string(),
            created_at: now(),
        };
        let j = serde_json::to_string(&lr).unwrap();
        assert!(j.contains("\"stableId\""), "expected camelCase: {j}");
        let back: LawRef = serde_json::from_str(&j).unwrap();
        assert_eq!(back.stable_id, lr.stable_id);
        assert_eq!(back.version_date, lr.version_date);
        assert_eq!(back.id(), lr.id);
    }

    /// The struct's `stable_id` must parse via the D8 parser, and `content_hash` of its
    /// `body_snapshot` must be a 64-char hex (data/02 §2.3 / §2.4 link).
    #[test]
    fn law_ref_stable_id_parses_and_body_hashes() {
        let stable_id = "law:中华人民共和国社会保险法/v2018-12-29/§63";
        let parts = parse(stable_id).unwrap();
        assert_eq!(parts.article, 63);
        assert_eq!(to_stable_id(&parts), stable_id);

        let h = content_hash("中华人民共和国境内的用人单位");
        assert_eq!(h.len(), 64);
    }
}
