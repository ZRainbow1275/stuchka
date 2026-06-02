//! Case — root aggregate (data/01 §1.1).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::enums::{CaseStatus, CoverageTier, DisputeSubtype, IdentityType};
use crate::time::{PlainDate, Timestamp};
use crate::traits::{AuditSubject, Identified, Versioned};

/// Case root aggregate (data/01 §1.1.6). Freezes KB version on creation (INV-04);
/// after freeze, modification = re-creation with a new `id` (§4.1.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Case {
    pub id: Uuid,
    pub identity_type: IdentityType,
    pub dispute_subtype: DisputeSubtype,
    /// `LD-XX-YY` (20 大类 85 子类, §4.1.2).
    pub dispute_category: String,
    pub coverage_tier: CoverageTier,
    pub case_occurred_at: PlainDate,
    /// GB/T 2260 first 2 digits.
    pub province: String,
    /// GB/T 2260 first 4 digits.
    pub city: String,
    /// Optional GB/T 2260 six-digit county code.
    pub region_code: Option<String>,
    /// SHA-256 hex 64 (INV-04 freeze hash).
    pub kb_version_hash: String,
    pub kb_version_label: String,
    pub status: CaseStatus,
    pub group_id: Option<Uuid>,
    pub dialogue_template_id: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub frozen_at: Option<Timestamp>,
}

impl Identified for Case {
    fn id(&self) -> Uuid {
        self.id
    }
}

impl Versioned for Case {
    fn kb_version_hash(&self) -> &str {
        &self.kb_version_hash
    }
    fn schema_version(&self) -> u32 {
        crate::SCHEMA_VERSION
    }
}

impl crate::traits::Auditable for Case {
    fn audit_subject(&self) -> AuditSubject {
        AuditSubject::System {
            component: "case".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::new_id;
    use crate::time::now;

    fn sample() -> Case {
        Case {
            id: new_id(),
            identity_type: IdentityType::StandardFullTime,
            dispute_subtype: DisputeSubtype::SocialInsWaiverInvalid,
            dispute_category: "LD-01-02".to_string(),
            coverage_tier: CoverageTier::MakeUsable,
            case_occurred_at: PlainDate::from_ymd_opt(2026, 9, 1).unwrap(),
            province: "44".to_string(),
            city: "4401".to_string(),
            region_code: Some("440106".to_string()),
            kb_version_hash: "0".repeat(64),
            kb_version_label: "2026-05-12-r1".to_string(),
            status: CaseStatus::Draft,
            group_id: None,
            dialogue_template_id: None,
            created_at: now(),
            updated_at: now(),
            frozen_at: None,
        }
    }

    #[test]
    fn case_serde_roundtrip_camel_case() {
        let c = sample();
        let j = serde_json::to_string(&c).unwrap();
        assert!(
            j.contains("\"identityType\""),
            "expected camelCase keys: {j}"
        );
        assert!(j.contains("\"caseOccurredAt\":\"2026-09-01\""), "got {j}");
        let back: Case = serde_json::from_str(&j).unwrap();
        assert_eq!(back.id, c.id);
        assert_eq!(back.dispute_category, c.dispute_category);
        assert_eq!(back.case_occurred_at, c.case_occurred_at);
    }

    #[test]
    fn case_implements_identified_and_versioned() {
        let c = sample();
        assert_eq!(c.id(), c.id);
        assert_eq!(c.kb_version_hash().len(), 64);
        assert_eq!(c.schema_version(), crate::SCHEMA_VERSION);
    }
}
