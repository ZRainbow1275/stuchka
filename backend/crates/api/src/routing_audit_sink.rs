//! `AuditLogRoutingSink` — the LIVE data-export-guard audit sink (`compliance/02` §6, INV-06).
//!
//! `crates/ai-dispatcher` declares the [`RoutingAuditSink`] trait but must not link sqlx / the
//! `crates/audit` chain (D6 dependency direction). The api layer owns the concrete sink: it adapts
//! the dispatcher's [`RoutingAuditEntry`] onto an `audit.append(System, AiRoutingDecision, what)`
//! write into the independent `audit.sqlite` hash chain. EVERY routing decision the guard makes on
//! the live `/llm/query` dispatch is persisted here as the four-tuple `what` — not just blocks (§6:
//! 每次路由决策必须写入一条审计日志). `override_reason` is carried verbatim and never omitted (§7),
//! because `write_routing_audit` (upstream) refuses an invariant-violating decision before it ever
//! reaches this sink.

use ai_dispatcher::{DispatcherError, RoutingAuditEntry, RoutingAuditSink};
use async_trait::async_trait;
use audit::{AuditLog, AuditReason, Subject};
use uuid::Uuid;

/// Writes routing-decision audit records into the live `audit.sqlite` chain.
pub struct AuditLogRoutingSink {
    audit: AuditLog,
}

impl AuditLogRoutingSink {
    /// Wrap a cloned [`AuditLog`] handle (cheap: the log is `Clone` over an `Arc` pool).
    pub fn new(audit: AuditLog) -> Self {
        Self { audit }
    }

    /// Build the §6 `what` JSON object from a [`RoutingAuditEntry`].
    ///
    /// `case_id` is included ONLY when it parses as a UUID (the D9 soft reference enforced by the
    /// audit schema validator). A free-text query carries no case id; the routing reason is not
    /// case-scoped, so an absent / non-UUID id is simply omitted rather than failing the write.
    fn what_for(entry: &RoutingAuditEntry) -> serde_json::Value {
        let mut what = serde_json::json!({
            "event": "ai_routing_decision",
            "request_id": entry.request_id,
            "input_max_grade": entry.input_max_grade,
            "high_sensitive_hit": entry.high_sensitive_hit,
            "user_choice": entry.user_choice,
            "actual_route": entry.actual_route,
            "override_reason": entry.override_reason,
            "desensitization_applied": entry.desensitization_applied,
            "model_id": entry.model_id,
            "kb_hash": entry.kb_hash,
        });
        if let Ok(id) = Uuid::parse_str(&entry.case_id) {
            what["case_id"] = serde_json::Value::String(id.to_string());
        }
        what
    }
}

#[async_trait]
impl RoutingAuditSink for AuditLogRoutingSink {
    async fn write_routing_audit(&self, entry: &RoutingAuditEntry) -> Result<(), DispatcherError> {
        let who = Subject::System {
            component: "ai_dispatcher_routing_guard".to_string(),
        };
        let what = Self::what_for(entry);
        self.audit
            .append(who, AuditReason::AiRoutingDecision, what)
            .await
            .map(|_seq| ())
            .map_err(|e| DispatcherError::Config(format!("routing audit append failed: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audit::{derive_audit_key, open_in_memory};

    fn entry() -> RoutingAuditEntry {
        RoutingAuditEntry {
            case_id: String::new(),
            request_id: "req-1".to_string(),
            input_max_grade: "L3".to_string(),
            high_sensitive_hit: true,
            user_choice: "overseas_cloud".to_string(),
            actual_route: "local_small_steel_cannon".to_string(),
            override_reason: Some("high_sensitive_force_local".to_string()),
            desensitization_applied: false,
            model_id: None,
            kb_hash: "kbhash".to_string(),
        }
    }

    #[test]
    fn what_omits_empty_case_id_but_keeps_override_reason() {
        let what = AuditLogRoutingSink::what_for(&entry());
        assert!(what.get("case_id").is_none(), "empty case id must be omitted");
        assert_eq!(what["event"], "ai_routing_decision");
        assert_eq!(what["override_reason"], "high_sensitive_force_local");
        assert_eq!(what["actual_route"], "local_small_steel_cannon");
        assert_eq!(what["high_sensitive_hit"], true);
    }

    #[test]
    fn what_includes_valid_case_id() {
        let id = Uuid::now_v7();
        let mut e = entry();
        e.case_id = id.to_string();
        let what = AuditLogRoutingSink::what_for(&e);
        assert_eq!(what["case_id"], id.to_string());
    }

    #[tokio::test]
    async fn appends_routing_decision_to_the_real_chain() {
        // A REAL audit.sqlite (in-memory) — the write must land in the hash chain and verify.
        let audit_key = derive_audit_key(&[7u8; 32]);
        let log = open_in_memory(&audit_key).await.expect("open audit log");
        let sink = AuditLogRoutingSink::new(log.clone());
        sink.write_routing_audit(&entry()).await.expect("write routing audit");

        let v = log.verify_chain().await.expect("verify chain");
        assert!(v.ok, "chain intact after routing-decision append");
        assert_eq!(v.checked, 1, "exactly one routing-decision record");
    }
}
