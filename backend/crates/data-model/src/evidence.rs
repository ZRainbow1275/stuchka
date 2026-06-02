//! Evidence — seven-class evidence with effective_score (data/01 §1.3).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{EvidenceCategory, EvidenceStatus};
use crate::time::Timestamp;
use crate::traits::Identified;

/// Evidence record. `effective_score` (0-1) is computed by the rule engine from five
/// dimensions (source / timing / completeness / relevance / authenticity) — INV-01 / INV-03.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Evidence {
    pub id: Uuid,
    pub case_id: Uuid,
    /// Seven classes (data/01 §1.3.3).
    pub evidence_type: EvidenceCategory,
    /// Local relative path (encrypted `.age` file).
    pub file_path: String,
    /// SHA-256 hex 64 (dedup + integrity).
    pub file_hash: String,
    pub mime_type: String,
    /// Single file <= 100MB (§5.1.3).
    pub byte_size: i64,
    pub effective_score: f32,
    /// `{source, timing, completeness, relevance, authenticity}` five keys, each 0-1.
    pub score_breakdown: serde_json::Value,
    pub status: EvidenceStatus,
    /// High-sensitivity detector hit (§5.2.3, crates/hsd).
    pub high_sensitivity: bool,
    pub collected_at: Option<Timestamp>,
    pub device_id: Option<String>,
    pub gps_coords: Option<serde_json::Value>,
    /// Chain membership: relation/wage/overtime/termination/work_injury.
    pub chain_membership: Vec<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Identified for Evidence {
    fn id(&self) -> Uuid {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::new_id;
    use crate::time::now;
    use serde_json::json;

    #[test]
    fn evidence_serde_roundtrip() {
        let e = Evidence {
            id: new_id(),
            case_id: new_id(),
            evidence_type: EvidenceCategory::AudioVideo,
            file_path: "evidence/abc.age".to_string(),
            file_hash: "a".repeat(64),
            mime_type: "audio/mpeg".to_string(),
            byte_size: 1024,
            effective_score: 0.75,
            score_breakdown: json!({
                "source": 0.8, "timing": 0.7, "completeness": 0.6,
                "relevance": 0.9, "authenticity": 0.8
            }),
            status: EvidenceStatus::Scored,
            high_sensitivity: true,
            collected_at: Some(now()),
            device_id: Some("dev-1".to_string()),
            gps_coords: Some(json!({"lat": 39.9, "lon": 116.4})),
            chain_membership: vec!["wage".to_string(), "overtime".to_string()],
            created_at: now(),
            updated_at: now(),
        };
        let j = serde_json::to_string(&e).unwrap();
        assert!(j.contains("\"evidenceType\":\"audio_video\""), "got {j}");
        assert!(j.contains("\"highSensitivity\":true"), "got {j}");
        let back: Evidence = serde_json::from_str(&j).unwrap();
        assert_eq!(back.chain_membership, e.chain_membership);
        assert_eq!(back.id(), e.id);
    }
}
