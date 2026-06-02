//! Group — R1 async group case (data/01 §1.6).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::time::Timestamp;
use crate::traits::Identified;

/// Group root: one initiator + N evidence contributors (§3.4.1 + §4.9).
/// R1a reserves fields only; R1b ships import/export.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Group {
    pub id: Uuid,
    pub case_id: Uuid,
    pub initiator_id: Uuid,
    /// Mandatory INV-10 confirmation popup time.
    pub inv10_confirmed_at: Timestamp,
    pub created_at: Timestamp,
}

impl Identified for Group {
    fn id(&self) -> Uuid {
        self.id
    }
}

/// Group ↔ contributor association (data/01 §1.6.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct GroupContributor {
    pub group_id: Uuid,
    pub contributor_id: Uuid,
    pub authorization_chain: serde_json::Value,
    pub joined_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::new_id;
    use crate::time::now;
    use serde_json::json;

    #[test]
    fn group_serde_roundtrip() {
        let g = Group {
            id: new_id(),
            case_id: new_id(),
            initiator_id: new_id(),
            inv10_confirmed_at: now(),
            created_at: now(),
        };
        let j = serde_json::to_string(&g).unwrap();
        assert!(j.contains("\"caseId\""), "expected camelCase: {j}");
        assert!(j.contains("\"inv10ConfirmedAt\""), "got {j}");
        let back: Group = serde_json::from_str(&j).unwrap();
        assert_eq!(back.id(), g.id);
        assert_eq!(back.initiator_id, g.initiator_id);
    }

    #[test]
    fn group_contributor_serde_roundtrip() {
        let gc = GroupContributor {
            group_id: new_id(),
            contributor_id: new_id(),
            authorization_chain: json!([{"from": "a", "to": "b"}]),
            joined_at: now(),
        };
        let j = serde_json::to_string(&gc).unwrap();
        assert!(j.contains("\"groupId\""), "got {j}");
        let back: GroupContributor = serde_json::from_str(&j).unwrap();
        assert_eq!(back.group_id, gc.group_id);
    }
}
