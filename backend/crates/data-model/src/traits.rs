//! Common traits over first-class objects (data/01 §1.0.3).

use uuid::Uuid;

/// Anything carrying a v7 UUID primary key (data/01 §1.0.3).
pub trait Identified {
    fn id(&self) -> Uuid;
}

/// Anything frozen against a knowledge-base version (INV-04, data/01 §1.0.3).
pub trait Versioned {
    /// INV-04 case-freeze hash (naming aligns with §0.6 `kb_version_hash`).
    fn kb_version_hash(&self) -> &str;
    fn schema_version(&self) -> u32;
}

/// Audit subject mapping for the four-tuple who/what (data/01 §1.0.3, mirrors data/04 §4.2.1).
///
/// data-model is a leaf crate, so it owns this lightweight `AuditSubject` rather than
/// depending on `crates/audit`; the audit crate maps to its own `Subject` at the boundary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditSubject {
    User {
        user_id: Uuid,
    },
    AiModel {
        provider: String,
        model: String,
        version: String,
    },
    RuleEngine {
        module: String,
        version: String,
    },
    System {
        component: String,
    },
    GroupContributor {
        contributor_id: Uuid,
    },
}

/// Anything that maps to an [`AuditSubject`] for INV-06 four-tuple logging (data/01 §1.0.3).
pub trait Auditable {
    fn audit_subject(&self) -> AuditSubject;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_subject_tagged_snake_case() {
        let s = AuditSubject::System {
            component: "kb_sync".into(),
        };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"kind\":\"system\""), "got {j}");
        let back: AuditSubject = serde_json::from_str(&j).unwrap();
        assert_eq!(s, back);
    }
}
