//! §4.2.1 — `Subject` (the `who` of the four-tuple).
//!
//! Owned by `crates/audit` (the `append` API owner — cross-crate reconciliation §B). data-model
//! keeps a lightweight `AuditSubject` for leaf-crate ergonomics; this crate maps to/from it at the
//! boundary via [`Subject::from_audit_subject`] / [`Subject::to_audit_subject`].

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The operating subject (`who`) of an audit record (data/04 §4.2.1).
///
/// Serialised internally-tagged on `kind` with `snake_case` variant names so the persisted
/// `who_kind` column equals the serde tag literal (`user | ai_model | rule_engine | system |
/// group_contributor`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Subject {
    /// The local user (default subject).
    User { user_id: Uuid },
    /// An AI model that produced an output (provenance for INV-06 / INV-08).
    AiModel {
        provider: String,
        model: String,
        version: String,
    },
    /// The deterministic rule engine (M9 / M5).
    RuleEngine { module: String, version: String },
    /// A system component (upgrade / backup / KB sync).
    System { component: String },
    /// A group-case contributor (R1b group features).
    GroupContributor { contributor_id: Uuid },
}

impl Subject {
    /// The `who_kind` text persisted in the `who_kind` column — equals the serde tag literal.
    #[must_use]
    pub fn who_kind(&self) -> &'static str {
        match self {
            Subject::User { .. } => "user",
            Subject::AiModel { .. } => "ai_model",
            Subject::RuleEngine { .. } => "rule_engine",
            Subject::System { .. } => "system",
            Subject::GroupContributor { .. } => "group_contributor",
        }
    }

    /// Map the data-model boundary type (`AuditSubject`) into the audit-owned `Subject`.
    #[must_use]
    pub fn from_audit_subject(s: data_model::AuditSubject) -> Self {
        match s {
            data_model::AuditSubject::User { user_id } => Subject::User { user_id },
            data_model::AuditSubject::AiModel {
                provider,
                model,
                version,
            } => Subject::AiModel {
                provider,
                model,
                version,
            },
            data_model::AuditSubject::RuleEngine { module, version } => {
                Subject::RuleEngine { module, version }
            }
            data_model::AuditSubject::System { component } => Subject::System { component },
            data_model::AuditSubject::GroupContributor { contributor_id } => {
                Subject::GroupContributor { contributor_id }
            }
        }
    }

    /// Project back into the data-model boundary type for DTO exposure.
    #[must_use]
    pub fn to_audit_subject(&self) -> data_model::AuditSubject {
        match self.clone() {
            Subject::User { user_id } => data_model::AuditSubject::User { user_id },
            Subject::AiModel {
                provider,
                model,
                version,
            } => data_model::AuditSubject::AiModel {
                provider,
                model,
                version,
            },
            Subject::RuleEngine { module, version } => {
                data_model::AuditSubject::RuleEngine { module, version }
            }
            Subject::System { component } => data_model::AuditSubject::System { component },
            Subject::GroupContributor { contributor_id } => {
                data_model::AuditSubject::GroupContributor { contributor_id }
            }
        }
    }

    /// The DTO `who` string (backend/01 §1.8): `user_id` for a user, otherwise the `who_kind`.
    #[must_use]
    pub fn who_display(&self) -> String {
        match self {
            Subject::User { user_id } => user_id.to_string(),
            other => other.who_kind().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn who_kind_matches_serde_tag() {
        let s = Subject::System {
            component: "kb_sync".into(),
        };
        assert_eq!(s.who_kind(), "system");
        let j = serde_json::to_value(&s).unwrap();
        assert_eq!(j["kind"], "system");
    }

    #[test]
    fn all_kinds_have_text() {
        let cases = [
            Subject::User {
                user_id: Uuid::now_v7(),
            },
            Subject::AiModel {
                provider: "deepseek".into(),
                model: "v3".into(),
                version: "1".into(),
            },
            Subject::RuleEngine {
                module: "m9".into(),
                version: "1".into(),
            },
            Subject::System {
                component: "backup".into(),
            },
            Subject::GroupContributor {
                contributor_id: Uuid::now_v7(),
            },
        ];
        let expected = [
            "user",
            "ai_model",
            "rule_engine",
            "system",
            "group_contributor",
        ];
        for (c, e) in cases.iter().zip(expected) {
            assert_eq!(c.who_kind(), e);
        }
    }

    #[test]
    fn audit_subject_round_trips() {
        let id = Uuid::now_v7();
        let s = Subject::User { user_id: id };
        let back = Subject::from_audit_subject(s.to_audit_subject());
        assert_eq!(s, back);
    }

    #[test]
    fn who_display_is_user_id_for_user() {
        let id = Uuid::now_v7();
        let s = Subject::User { user_id: id };
        assert_eq!(s.who_display(), id.to_string());
        let sys = Subject::System {
            component: "x".into(),
        };
        assert_eq!(sys.who_display(), "system");
    }
}
