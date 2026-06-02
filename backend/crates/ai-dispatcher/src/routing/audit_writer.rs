//! 路由决策审计写入 (`compliance/02` §6, INV-06).
//!
//! Every routing decision MUST write one audit record (`event: ai_routing_decision`) carrying the
//! who/when/why/what four-tuple. The audit SINK is the independent `audit.sqlite` (`crates/audit`,
//! D3) — but `ai-dispatcher` must not link sqlx, so this module builds the structured
//! [`RoutingAuditEntry`] (the `what`) and writes it through a [`RoutingAuditSink`] trait the api
//! layer implements over `crates/audit` (the same adapter pattern as `KbContext` / `RuleContext`).
//! `override_reason` is NEVER omitted when the guard overrode the user's choice (§7).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::DispatcherError;
use crate::routing::decision::RoutingDecision;

/// The `what` payload of an `ai_routing_decision` audit record (`compliance/02` §6).
///
/// Field names mirror the spec JSON exactly: `input_max_grade` / `high_sensitive_hit` /
/// `user_choice` / `actual_route` / `override_reason` / `desensitization_applied` / `model_id` /
/// `kb_hash` (+ `case_id` so the record is case-scoped for `query_by_case`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingAuditEntry {
    /// Soft reference to the case (D9; required by the case-scoped audit schema).
    pub case_id: String,
    /// `request_id` — the `why` correlation id (`compliance/02` §6 `why: user_request_id`).
    pub request_id: String,
    /// L1..L4 highest grade across the payload.
    pub input_max_grade: String,
    /// Whether the high-sensitivity detector hit.
    pub high_sensitive_hit: bool,
    /// The user's chosen channel.
    pub user_choice: String,
    /// The guard's final route.
    pub actual_route: String,
    /// Override reason — present whenever `actual_route != user_choice` (§7 禁止省略).
    pub override_reason: Option<String>,
    /// Whether the domestic-cloud desensitisation pipeline ran.
    pub desensitization_applied: bool,
    /// The concrete model id used (e.g. `qwen2.5-7b-q4-2026Q1`); `None` until selected.
    pub model_id: Option<String>,
    /// The active KB version hash (so a later KB rollback can be reconciled, §8).
    pub kb_hash: String,
}

impl RoutingAuditEntry {
    /// Build the audit entry from a [`RoutingDecision`] + request/case context.
    pub fn from_decision(
        decision: &RoutingDecision,
        case_id: impl Into<String>,
        request_id: impl Into<String>,
        model_id: Option<String>,
        kb_hash: impl Into<String>,
    ) -> Self {
        Self {
            case_id: case_id.into(),
            request_id: request_id.into(),
            input_max_grade: format!("{:?}", decision.input_max_grade),
            high_sensitive_hit: decision.hsd_hit,
            user_choice: decision.user_choice.as_str().to_string(),
            actual_route: decision.actual_route.as_str().to_string(),
            override_reason: decision.override_reason.map(|s| s.to_string()),
            desensitization_applied: decision.desensitization_applied,
            model_id,
            kb_hash: kb_hash.into(),
        }
    }

    /// Render the four-tuple `what` as JSON (the value handed to the audit sink).
    pub fn to_what(&self) -> serde_json::Value {
        serde_json::json!({
            "case_id": self.case_id,
            "input_max_grade": self.input_max_grade,
            "high_sensitive_hit": self.high_sensitive_hit,
            "user_choice": self.user_choice,
            "actual_route": self.actual_route,
            "override_reason": self.override_reason,
            "desensitization_applied": self.desensitization_applied,
            "model_id": self.model_id,
            "kb_hash": self.kb_hash,
        })
    }
}

/// The audit sink the routing path writes through. The api layer implements it over
/// `crates/audit` (`AuditLog::append`, reason `ai_routing_decision`-class); tests implement an
/// in-memory recorder. Keeps `ai-dispatcher` free of a direct sqlx / `crates/audit` dependency.
#[async_trait]
pub trait RoutingAuditSink: Send + Sync {
    /// Persist one routing-decision audit record (who/when/why/what). The sink supplies `who`
    /// (system) and `when` (Utc::now) itself; this passes the `why` correlation + the `what`.
    async fn write_routing_audit(&self, entry: &RoutingAuditEntry) -> Result<(), DispatcherError>;
}

/// Write a routing-decision audit record through `sink` (`compliance/02` §6). Refuses to write a
/// decision that violates the INV-05 invariants (an overridden decision with no reason, etc.) so a
/// non-compliant record can never reach the chain (§7).
pub async fn write_routing_audit(
    sink: &dyn RoutingAuditSink,
    decision: &RoutingDecision,
    case_id: &str,
    request_id: &str,
    model_id: Option<String>,
    kb_hash: &str,
) -> Result<(), DispatcherError> {
    decision
        .assert_invariants()
        .map_err(|m| DispatcherError::Config(format!("routing decision violates INV-05: {m}")))?;
    let entry = RoutingAuditEntry::from_decision(decision, case_id, request_id, model_id, kb_hash);
    sink.write_routing_audit(&entry).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::decision::Route;
    use data_model::DataGrade;
    use std::sync::Mutex;

    struct MemSink {
        entries: Mutex<Vec<RoutingAuditEntry>>,
    }

    #[async_trait]
    impl RoutingAuditSink for MemSink {
        async fn write_routing_audit(
            &self,
            entry: &RoutingAuditEntry,
        ) -> Result<(), DispatcherError> {
            self.entries.lock().unwrap().push(entry.clone());
            Ok(())
        }
    }

    fn forced_local_decision() -> RoutingDecision {
        RoutingDecision {
            actual_route: Route::LocalSmallSteelCannon,
            input_max_grade: DataGrade::L3,
            hsd_hit: true,
            scene_forced_local: false,
            user_choice: Route::OverseasCloud,
            override_reason: Some("high_sensitive_force_local"),
            desensitization_applied: false,
            requires_second_confirm: false,
            matched_rule: "R2",
        }
    }

    #[tokio::test]
    async fn writes_four_tuple_with_override_reason() {
        let sink = MemSink {
            entries: Mutex::new(Vec::new()),
        };
        let d = forced_local_decision();
        write_routing_audit(&sink, &d, "case-1", "req-1", None, "kbhash")
            .await
            .unwrap();
        let entries = sink.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        let what = entries[0].to_what();
        assert_eq!(what["actual_route"], "local_small_steel_cannon");
        assert_eq!(what["user_choice"], "overseas_cloud");
        assert_eq!(what["override_reason"], "high_sensitive_force_local");
        assert_eq!(what["high_sensitive_hit"], true);
    }

    #[tokio::test]
    async fn refuses_to_write_invariant_violation() {
        let sink = MemSink {
            entries: Mutex::new(Vec::new()),
        };
        let mut d = forced_local_decision();
        d.override_reason = None; // overridden but no reason → must be refused (§7)
        let res = write_routing_audit(&sink, &d, "case-1", "req-1", None, "kbhash").await;
        assert!(res.is_err());
        assert!(sink.entries.lock().unwrap().is_empty());
    }
}
