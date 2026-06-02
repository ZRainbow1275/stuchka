//! CaseFact — fact with state machine (data/01 §1.2).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{CoverageTag, FactCategory, FactSource, FactStatus};
use crate::time::Timestamp;
use crate::traits::Identified;

/// A single case fact with its own lifecycle (data/01 §1.2.6). Each AI-inferred fact
/// requires user review (INV-06 four-tuple).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct CaseFact {
    pub id: Uuid,
    pub case_id: Uuid,
    pub content: String,
    pub category: FactCategory,
    pub status: FactStatus,
    pub source: FactSource,
    /// 0-1; required when `source = ai_inferred`, otherwise None.
    pub confidence: Option<f32>,
    pub coverage_tag: CoverageTag,
    /// Soft references to `evidence.id` (array, no FK; repository-layer validated).
    pub evidence_refs: Vec<Uuid>,
    pub group_id: Option<Uuid>,
    pub contributor_id: Option<Uuid>,
    pub authorization_chain: Option<serde_json::Value>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Identified for CaseFact {
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
    fn fact_serde_roundtrip() {
        let f = CaseFact {
            id: new_id(),
            case_id: new_id(),
            content: "未签订书面劳动合同".to_string(),
            category: FactCategory::RelationQualification,
            status: FactStatus::Pending,
            source: FactSource::AiInferred,
            confidence: Some(0.82),
            coverage_tag: CoverageTag::Approximate,
            evidence_refs: vec![new_id(), new_id()],
            group_id: None,
            contributor_id: None,
            authorization_chain: None,
            created_at: now(),
            updated_at: now(),
        };
        let j = serde_json::to_string(&f).unwrap();
        assert!(j.contains("\"caseId\""), "expected camelCase: {j}");
        assert!(j.contains("\"coverageTag\":\"approximate\""), "got {j}");
        let back: CaseFact = serde_json::from_str(&j).unwrap();
        assert_eq!(back.evidence_refs, f.evidence_refs);
        assert_eq!(back.confidence, f.confidence);
        assert_eq!(back.id(), f.id);
    }
}
