//! Three-stage A→F integration (ai/01 §1.9, brief acceptance #4):
//! - HSD force-local → cloud sees 0 requests;
//! - Level4 → compensation refused, never falls back;
//! - rule abstention-first → a determinate `RuleOutcome::Ok` discards Stage E AI output;
//! - rule abstention (`Unknown`) → AI confidence capped at 0.49 → Low bucket.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ai_dispatcher::stage::{answer, HsdContext, KbContext, RuleContext, StageContext};
use ai_dispatcher::{
    CompleteRequest, CompleteResponse, ConfBucket, DegradeLevel, DispatcherError, HealthCheck,
    Inv08Templates, Provider, ProviderId, RateLimit, UserQuery,
};
use async_trait::async_trait;
use data_model::{CoverageTag, RouteHint};
use rust_decimal::Decimal;

// ---- mocks ------------------------------------------------------------------------------------

struct MockHsd {
    hint: RouteHint,
    high: bool,
}
impl HsdContext for MockHsd {
    fn scan(&self, _t: &str) -> (RouteHint, bool) {
        (self.hint, self.high)
    }
}

struct MockKb {
    age: u32,
    hits: Vec<(String, f32)>,
}
#[async_trait]
impl KbContext for MockKb {
    async fn pull_incremental_if_due(&self) -> Result<(), DispatcherError> {
        Ok(())
    }
    fn age_days(&self) -> u32 {
        self.age
    }
    fn hybrid_query(&self, _t: &str, _k: usize) -> Vec<(String, f32)> {
        self.hits.clone()
    }
    fn version_label(&self) -> String {
        "2026-05-12-r1".to_string()
    }
}

struct MockRules {
    outcome: Option<rule_engine::RuleOutcome>,
}
impl RuleContext for MockRules {
    fn try_resolve(&self, _r: &rule_engine::RuleRequest) -> Option<rule_engine::RuleOutcome> {
        self.outcome.clone()
    }
}

/// A cloud provider that counts how many times `complete` is called (must be 0 when force-local).
struct CountingProvider {
    id: ProviderId,
    calls: Arc<AtomicUsize>,
}
#[async_trait]
impl Provider for CountingProvider {
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
    async fn complete(&self, _req: CompleteRequest) -> Result<CompleteResponse, DispatcherError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CompleteResponse {
            content: "云端回答".to_string(),
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

fn rule_request() -> rule_engine::RuleRequest {
    rule_engine::RuleRequest {
        intent: rule_engine::RuleIntent::Severance,
        facts: Default::default(),
        deadline_facts: None,
        province: "44".to_string(),
        city: "4403".to_string(),
        severance_pre_tax: None,
    }
}

fn ok_exact_outcome() -> rule_engine::RuleOutcome {
    rule_engine::RuleOutcome::Ok {
        value: rule_engine::ComputedValue::Money(Decimal::from(128000)),
        coverage_tag: CoverageTag::Exact,
        law_refs: vec![],
        derivation: vec![rule_engine::DerivationStep::new(
            "经济赔偿金",
            "8000 × 8 × 2",
            Decimal::from(128000),
        )],
    }
}

fn unknown_outcome() -> rule_engine::RuleOutcome {
    rule_engine::RuleOutcome::OutOfScope {
        coverage_tag: CoverageTag::Unknown,
        reasons: vec!["缺少关键事实".to_string()],
        next_actions: vec![
            rule_engine::RuleNextAction::CollectFact {
                field: "入职日期".to_string(),
            },
            rule_engine::RuleNextAction::CollectFact {
                field: "解除原因".to_string(),
            },
        ],
    }
}

// ---- tests ------------------------------------------------------------------------------------

#[tokio::test]
async fn hsd_force_local_sends_zero_cloud_requests() {
    let calls = Arc::new(AtomicUsize::new(0));
    let primary = CountingProvider {
        id: ProviderId::DeepSeek,
        calls: calls.clone(),
    };
    let secondary = CountingProvider {
        id: ProviderId::QwenCloud,
        calls: calls.clone(),
    };
    // No local model: force-local with strong hit must BLOCK, never reach the cloud.
    let hsd = MockHsd {
        hint: RouteHint::ForceLocal,
        high: true,
    };
    let kb = MockKb {
        age: 1,
        hits: vec![],
    };
    let rules = MockRules { outcome: None };
    let templates = Inv08Templates::default();

    let ctx = StageContext {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        level: DegradeLevel::Level0,
        primary_id: ProviderId::DeepSeek,
        secondary_id: ProviderId::QwenCloud,
        primary: Some(&primary),
        secondary: Some(&secondary),
        local: None,
        templates: &templates,
        rule_request: None,
        overseas_enabled: false,
        audit: None,
    };

    let q = UserQuery::new("我的电话是13800138000帮我看看赔偿");
    let a = answer(&q, &ctx).await;

    assert_eq!(calls.load(Ordering::SeqCst), 0, "cloud must see 0 requests");
    assert!(a.pii_blocked, "HSD strong hit + no local → blocked");
    assert!(a.abstained);
}

#[tokio::test]
async fn level4_refuses_and_never_falls_back() {
    let calls = Arc::new(AtomicUsize::new(0));
    let primary = CountingProvider {
        id: ProviderId::DeepSeek,
        calls: calls.clone(),
    };
    let hsd = MockHsd {
        hint: RouteHint::Auto,
        high: false,
    };
    // KB older than 30 days → Level4 gate even though level is nominally Level0.
    let kb = MockKb {
        age: 45,
        hits: vec![],
    };
    let rules = MockRules { outcome: None };
    let templates = Inv08Templates::default();
    let ctx = StageContext {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        level: DegradeLevel::Level0,
        primary_id: ProviderId::DeepSeek,
        secondary_id: ProviderId::QwenCloud,
        primary: Some(&primary),
        secondary: None,
        local: None,
        templates: &templates,
        rule_request: None,
        overseas_enabled: false,
        audit: None,
    };
    let a = answer(&UserQuery::new("赔偿多少"), &ctx).await;
    assert_eq!(a.fallback_level, 4);
    assert!(a.abstained, "Level4 refuses, no AI fallback");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "no cloud at Level4");
}

#[tokio::test]
async fn rule_ok_discards_ai_output() {
    // A determinate rule Ok(Exact) means Stage E AI output is discarded (INV-01).
    let calls = Arc::new(AtomicUsize::new(0));
    let primary = CountingProvider {
        id: ProviderId::DeepSeek,
        calls: calls.clone(),
    };
    let hsd = MockHsd {
        hint: RouteHint::Auto,
        high: false,
    };
    let kb = MockKb {
        age: 1,
        hits: vec![],
    };
    let rules = MockRules {
        outcome: Some(ok_exact_outcome()),
    };
    let templates = Inv08Templates::default();
    let ctx = StageContext {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        level: DegradeLevel::Level0,
        primary_id: ProviderId::DeepSeek,
        secondary_id: ProviderId::QwenCloud,
        primary: Some(&primary),
        secondary: None,
        local: None,
        templates: &templates,
        rule_request: Some(rule_request()),
        overseas_enabled: false,
        audit: None,
    };
    let a = answer(&UserQuery::new("8年工龄违法解除赔偿多少"), &ctx).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "rule Ok must short-circuit before Stage E"
    );
    assert_eq!(a.source_tag, data_model::SourceTag::Rule);
    assert_eq!(a.inv08_bucket, ConfBucket::High);
    assert!(a.content.contains("规则引擎"));
    assert!(!a.abstained);
}

#[tokio::test]
async fn rule_unknown_caps_ai_into_low_bucket() {
    // Rule abstention (Unknown): AI still runs but confidence is capped 0.49 → Low (INV-01).
    let calls = Arc::new(AtomicUsize::new(0));
    let primary = CountingProvider {
        id: ProviderId::DeepSeek,
        calls: calls.clone(),
    };
    let hsd = MockHsd {
        hint: RouteHint::Auto,
        high: false,
    };
    let kb = MockKb {
        age: 1,
        hits: vec![("law:测试/§1".to_string(), 9.0)],
    };
    let rules = MockRules {
        outcome: Some(unknown_outcome()),
    };
    let templates = Inv08Templates::default();
    let ctx = StageContext {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        level: DegradeLevel::Level0,
        primary_id: ProviderId::DeepSeek,
        secondary_id: ProviderId::QwenCloud,
        primary: Some(&primary),
        secondary: None,
        local: None,
        templates: &templates,
        rule_request: Some(rule_request()),
        overseas_enabled: false,
        audit: None,
    };
    let a = answer(&UserQuery::new("我这种情况能要赔偿吗"), &ctx).await;
    // AI was consulted (Unknown does not short-circuit) but capped into Low.
    assert_eq!(calls.load(Ordering::SeqCst), 1, "AI runs under Unknown");
    assert_eq!(a.inv08_bucket, ConfBucket::Low);
    assert!(a.confidence <= 0.49, "capped at 0.49, got {}", a.confidence);
    assert!(a.heuristic_followups.len() >= 3, "Low needs >= 3 key facts");
    assert!(a.content.contains("再回来问我"));
    assert!(!a.abstained, "Low is a follow-up, not a refusal");
}

#[tokio::test]
async fn high_confidence_cloud_answer_composes() {
    // No rule, fresh KB, healthy cloud, Level0 → AI High/Mid answer.
    let calls = Arc::new(AtomicUsize::new(0));
    let primary = CountingProvider {
        id: ProviderId::DeepSeek,
        calls: calls.clone(),
    };
    let hsd = MockHsd {
        hint: RouteHint::Auto,
        high: false,
    };
    let kb = MockKb {
        age: 1,
        hits: vec![("law:劳动合同法/§47".to_string(), 12.0)],
    };
    let rules = MockRules { outcome: None };
    let templates = Inv08Templates::default();
    let ctx = StageContext {
        hsd: &hsd,
        kb: &kb,
        rules: &rules,
        level: DegradeLevel::Level0,
        primary_id: ProviderId::DeepSeek,
        secondary_id: ProviderId::QwenCloud,
        primary: Some(&primary),
        secondary: None,
        local: None,
        templates: &templates,
        rule_request: None,
        overseas_enabled: false,
        audit: None,
    };
    let a = answer(&UserQuery::new("加班费基数怎么算"), &ctx).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "cloud consulted");
    assert!(!a.abstained);
    assert!(!a.evidence_chain.is_empty(), "KB hits surfaced as evidence");
}
