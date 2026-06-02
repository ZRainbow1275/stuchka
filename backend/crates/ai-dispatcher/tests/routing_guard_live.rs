//! INV-05 data-export guard, LIVE-path integration (`compliance/02` §3 / §6).
//!
//! These tests drive the REAL [`ai_dispatcher::stage::answer`] A→F pipeline with the GENUINE
//! [`ai_dispatcher::wiring::HsdAdapter`] over a real [`hsd::HsdDetector`], proving the data-export
//! guard runs on the live dispatch (not dead code):
//!
//! - a high-grade-PII request (L4 structured field) is FORCED LOCAL and, with no local model, is
//!   BLOCKED — the cloud provider sees ZERO requests (未通过禁止发出, §3 + §7);
//! - an L3-graded request that DOES go to the domestic cloud is DESENSITISED first — the masked
//!   payload (not the raw phone) is what the provider receives (§3.4);
//! - EVERY decision is written to the audit sink as the §6 four-tuple, carrying `override_reason`
//!   whenever the guard overrode the user's channel (INV-06 / §7).

use std::sync::Mutex;

use ai_dispatcher::stage::{answer, KbContext, RuleContext, StageContext};
use ai_dispatcher::wiring::HsdAdapter;
use ai_dispatcher::{
    CompleteRequest, CompleteResponse, DegradeLevel, DispatcherError, HealthCheck, Inv08Templates,
    Provider, ProviderId, RateLimit, RoutingAuditEntry, RoutingAuditSink, UserQuery,
};
use async_trait::async_trait;
use hsd::HsdDetector;

// ---- in-memory audit sink (records the §6 four-tuple) -----------------------------------------

struct RecordingSink {
    entries: Mutex<Vec<RoutingAuditEntry>>,
}
impl RecordingSink {
    fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }
    fn entries(&self) -> Vec<RoutingAuditEntry> {
        self.entries.lock().unwrap().clone()
    }
}
#[async_trait]
impl RoutingAuditSink for RecordingSink {
    async fn write_routing_audit(&self, entry: &RoutingAuditEntry) -> Result<(), DispatcherError> {
        self.entries.lock().unwrap().push(entry.clone());
        Ok(())
    }
}

// ---- a cloud provider that captures EXACTLY what payload it was asked to send ------------------

struct CapturingProvider {
    id: ProviderId,
    sent: Mutex<Vec<String>>,
}
impl CapturingProvider {
    fn new(id: ProviderId) -> Self {
        Self {
            id,
            sent: Mutex::new(Vec::new()),
        }
    }
    fn sent(&self) -> Vec<String> {
        self.sent.lock().unwrap().clone()
    }
}
#[async_trait]
impl Provider for CapturingProvider {
    fn id(&self) -> &'static str {
        self.id.as_str()
    }
    fn is_cn_jurisdiction(&self) -> bool {
        true
    }
    async fn health(&self) -> Result<HealthCheck, DispatcherError> {
        Ok(HealthCheck {
            provider: self.id,
            healthy: true,
            latency_ms: 1,
            model: None,
        })
    }
    async fn complete(&self, req: CompleteRequest) -> Result<CompleteResponse, DispatcherError> {
        // Record the OUTBOUND user content the dispatcher handed us (the on-wire payload).
        self.sent.lock().unwrap().push(req.user.clone());
        Ok(CompleteResponse {
            content: "境内云回答".to_string(),
            provider: self.id,
            model: "mock".to_string(),
            self_reported_confidence: Some(0.9),
            total_tokens: Some(10),
        })
    }
    fn rate_limit_hint(&self) -> RateLimit {
        RateLimit { rpm: 1, tpm: 1 }
    }
}

// ---- KB / Rule mocks (fresh KB, no rule intent → AI answers directly) -------------------------

struct FreshKb;
#[async_trait]
impl KbContext for FreshKb {
    async fn pull_incremental_if_due(&self) -> Result<(), DispatcherError> {
        Ok(())
    }
    fn age_days(&self) -> u32 {
        1
    }
    fn hybrid_query(&self, _t: &str, _k: usize) -> Vec<(String, f32)> {
        Vec::new()
    }
    fn version_label(&self) -> String {
        "2026-05-12-r1".to_string()
    }
}

struct NoRules;
impl RuleContext for NoRules {
    fn try_resolve(&self, _r: &rule_engine::RuleRequest) -> Option<rule_engine::RuleOutcome> {
        None
    }
}

// -----------------------------------------------------------------------------------------------

/// A real detector (R1a regex-only strong-signal layer — phone / id-card / bank-card / path / email).
fn detector() -> HsdDetector {
    HsdDetector::new_regex_only()
}

/// The fixtures backing one live StageContext (bundled so the builder stays within the argument
/// budget — clippy::too_many_arguments). `local` is never available in these tests (no real weight
/// file) so the "forced local with no local model ⇒ blocked" path is exercised honestly.
struct LiveFixtures<'a> {
    hsd: &'a HsdAdapter<'a>,
    kb: &'a FreshKb,
    rules: &'a NoRules,
    primary: &'a dyn Provider,
    templates: &'a Inv08Templates,
    overseas_enabled: bool,
    audit: &'a dyn RoutingAuditSink,
}

/// Build the live StageContext with the GENUINE HsdAdapter + a domestic provider + an audit sink.
fn live_ctx<'a>(f: LiveFixtures<'a>) -> StageContext<'a> {
    let LiveFixtures {
        hsd,
        kb,
        rules,
        primary,
        templates,
        overseas_enabled,
        audit,
    } = f;
    StageContext {
        hsd,
        kb,
        rules,
        level: DegradeLevel::Level0,
        primary_id: ProviderId::DeepSeek,
        secondary_id: ProviderId::QwenCloud,
        primary: Some(primary),
        secondary: None,
        local: None,
        templates,
        rule_request: None,
        overseas_enabled,
        audit: Some(audit),
    }
}

#[tokio::test]
async fn l4_pii_request_is_forced_local_blocked_and_audited_not_sent() {
    // An id-card-number structured field grades the payload L4 (core privacy): the guard forces
    // local (R1) and overrides the user's domestic-cloud choice; with no local model the dispatch is
    // BLOCKED. The cloud provider must see ZERO requests, and the decision must be audited.
    let det = detector();
    let hsd = HsdAdapter { detector: &det };
    let kb = FreshKb;
    let rules = NoRules;
    let templates = Inv08Templates::default();
    let provider = CapturingProvider::new(ProviderId::DeepSeek);
    let sink = RecordingSink::new();

    let ctx = live_ctx(LiveFixtures {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        primary: &provider,
        templates: &templates,
        overseas_enabled: false,
        audit: &sink,
    });

    let mut q = UserQuery::new("帮我看看这个情况怎么处理"); // plain free text (no strong signal)
    q.structured_fields = vec!["id_card_number".to_string()]; // L4

    let a = answer(&q, &ctx).await;

    // The L4 grade forced local; no local model ⇒ blocked, nothing sent to any cloud (§7).
    assert!(a.pii_blocked, "L4 forced-local with no local model must block");
    assert!(a.abstained);
    assert_eq!(provider.sent().len(), 0, "the cloud provider must see ZERO requests");

    // EVERY decision is audited (§6): the four-tuple records the local route + the override reason.
    let entries = sink.entries();
    assert_eq!(entries.len(), 1, "exactly one routing-decision audit record");
    let e = &entries[0];
    assert_eq!(e.input_max_grade, "L4");
    assert_eq!(e.actual_route, "local_small_steel_cannon");
    assert_eq!(
        e.override_reason.as_deref(),
        Some("l4_force_local"),
        "§7: override_reason is never omitted when the guard overrides the user's choice"
    );
    assert!(!e.desensitization_applied, "L4 is never masked-and-sent (§7)");
}

#[tokio::test]
async fn hsd_hit_in_free_text_forces_local_over_domestic_and_is_audited() {
    // A real phone number in the free text trips the genuine hsd strong-signal layer (high=true,
    // ForceLocal). Even though the user chose the domestic cloud and the grade is only L2, the guard
    // forces local (R0/R2 force-local via the hsd hit). No local model ⇒ blocked, ZERO cloud sends.
    let det = detector();
    let hsd = HsdAdapter { detector: &det };
    let kb = FreshKb;
    let rules = NoRules;
    let templates = Inv08Templates::default();
    let provider = CapturingProvider::new(ProviderId::DeepSeek);
    let sink = RecordingSink::new();

    let ctx = live_ctx(LiveFixtures {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        primary: &provider,
        templates: &templates,
        overseas_enabled: false,
        audit: &sink,
    });

    // Free text carries a real mainland phone number → the LIVE detector hits.
    let q = UserQuery::new("我的电话是13800138000，公司拖欠工资，请帮我看看");
    let a = answer(&q, &ctx).await;

    assert!(a.pii_blocked, "hsd strong hit + no local model must block");
    assert_eq!(provider.sent().len(), 0, "hsd-hit request must NOT reach the cloud");

    let entries = sink.entries();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].high_sensitive_hit, "the live hsd hit is recorded in the audit");
    assert_eq!(entries[0].actual_route, "local_small_steel_cannon");
}

#[tokio::test]
async fn l3_clean_freetext_goes_domestic_desensitised_and_audited() {
    // L3 grade via a structured field (salary_breakdown), but the FREE TEXT carries no strong PII
    // signal → the live hsd does NOT hit. The guard's R3 path sends to the domestic cloud AFTER the
    // §3.4 desensitisation pass (desensitization_applied = true). The audit records the domestic
    // route + the applied desensitisation; the cloud is consulted exactly once.
    let det = detector();
    let hsd = HsdAdapter { detector: &det };
    let kb = FreshKb;
    let rules = NoRules;
    let templates = Inv08Templates::default();
    let provider = CapturingProvider::new(ProviderId::DeepSeek);
    let sink = RecordingSink::new();

    let ctx = live_ctx(LiveFixtures {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        primary: &provider,
        templates: &templates,
        overseas_enabled: false,
        audit: &sink,
    });

    let mut q = UserQuery::new("公司拖欠了我三个月的工资，应该怎么主张？"); // no strong signal
    q.structured_fields = vec!["salary_breakdown".to_string()]; // L3

    let a = answer(&q, &ctx).await;

    assert!(!a.abstained, "L3 clean text routes to the domestic cloud, not a block");
    assert_eq!(provider.sent().len(), 1, "domestic cloud consulted exactly once");

    let entries = sink.entries();
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.input_max_grade, "L3");
    assert_eq!(e.actual_route, "domestic_cloud");
    assert!(
        e.desensitization_applied,
        "§3.4: the domestic-cloud L3 path must run the desensitisation pipeline"
    );
}

#[tokio::test]
async fn l3_with_real_pii_in_text_is_desensitised_before_any_cloud_send() {
    // The strongest §3.4 proof: an L3-graded request whose FREE TEXT contains a real phone number.
    // The live hsd hits → the guard forces LOCAL (R2: L3 + hsd), so the raw phone NEVER reaches a
    // cloud. With no local model the dispatch is blocked; the cloud sees zero sends and the raw
    // phone string appears in NO outbound payload.
    let det = detector();
    let hsd = HsdAdapter { detector: &det };
    let kb = FreshKb;
    let rules = NoRules;
    let templates = Inv08Templates::default();
    let provider = CapturingProvider::new(ProviderId::DeepSeek);
    let sink = RecordingSink::new();

    // User CHOSE overseas this round (overseas_enabled + chosen) — the guard must still override it.
    let mut ctx = live_ctx(LiveFixtures {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        primary: &provider,
        templates: &templates,
        overseas_enabled: true,
        audit: &sink,
    });
    ctx.local = None;

    let mut q = UserQuery::new("我的电话13800138000，工资明细对不上，帮我算算");
    q.structured_fields = vec!["salary_breakdown".to_string()]; // L3
    q.allow_cross_border = true; // overseas_chosen

    let a = answer(&q, &ctx).await;

    assert!(a.pii_blocked, "L3 + hsd hit forces local; no local model ⇒ blocked");
    // No cloud send at all, and certainly not the raw phone number.
    for payload in provider.sent() {
        assert!(
            !payload.contains("13800138000"),
            "raw phone number must never appear in any outbound cloud payload"
        );
    }
    assert_eq!(provider.sent().len(), 0, "ZERO cloud requests (未通过禁止发出)");

    let entries = sink.entries();
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.actual_route, "local_small_steel_cannon");
    assert!(e.high_sensitive_hit);
    assert_eq!(
        e.override_reason.as_deref(),
        Some("high_sensitive_force_local"),
        "the guard overrode the user's overseas choice — reason recorded (§7)"
    );
}
