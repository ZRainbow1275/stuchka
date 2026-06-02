//! `ai-dispatcher` — M7 4-layer fallback + provider primary/secondary + INV-08 three-tier.
//!
//! 0529 spec crate (master-index §0.4 / D6; ai/01 + ai/03). In-process library (D1): the HTTP
//! `/llm` routes live in `crates/api`, the child-process spawn / READY handshake in `crates/core`.
//! This crate owns the domain logic the api handler calls:
//!
//! - [`levels`] — the Level0-4 degrade FSM + module availability matrix (60s tick, recover upward,
//!   Level4 compensation-refuse never falls back).
//! - [`provider`] / [`providers`] — the `Provider` trait + `ProviderId` + registry + the real
//!   SiliconFlow gateway clients (DeepSeek primary / Qwen secondary).
//! - [`stage`] — the three-stage `answer()` A→F pipeline.
//! - [`confidence`] / [`inv08_template`] / [`inv08_compose`] — INV-08 three-tier quantification.
//! - [`onboarding`] / [`confirm`] / [`observability`] — wizard, cloud-confirm protocol, degrade events.
//! - [`local_qwen`] — the local small-model fallback (load + integrity gate; inference deferred).
//!
//! Dependency direction (D6): `api → ai-dispatcher → {data-model, hsd, rule-engine, kb}`. This crate
//! MUST NOT appear in `rule-engine` / `hsd`'s dependency tree (INV-01 physical isolation).

pub mod answer;
pub mod confidence;
pub mod config;
pub mod confirm;
pub mod error;
pub mod inv08_compose;
pub mod inv08_template;
pub mod levels;
pub mod local_qwen;
pub mod observability;
pub mod onboarding;
pub mod provider;
pub mod providers;
pub mod routing;
pub mod stage;
pub mod wiring;

pub use answer::{Answer, EvidenceLink, ScoredAnswer, UserQuery};
pub use confidence::{bucket, compute_confidence, ConfBucket, ConfidenceInput};
pub use config::{LocalConfig, NetworkConfig, SecretsConfig, SiliconFlowConfig};
pub use confirm::{
    evaluate as evaluate_confirm, AiPurpose, CloudConfirmRequest, CloudConfirmResponse,
    ConfirmOutcome, HsdHitSummary, Jurisdiction,
};
pub use error::DispatcherError;
pub use inv08_compose::compose;
pub use inv08_template::Inv08Templates;
pub use levels::{
    module_matrix_for, DegradeLevel, HealthSnapshot, LevelMachine, ModuleId, ModuleMatrix,
    ModuleState, TransitionRecord,
};
pub use local_qwen::LocalQwen;
pub use observability::{DegradeEvent, ReasonCode, RecoveryHint};
pub use onboarding::{
    OnboardingChoice, OnboardingError, OnboardingSpec, OnboardingStep, OnboardingValidator,
};
pub use provider::{
    decide_switch, CompleteRequest, CompleteResponse, HealthCheck, Provider, ProviderId,
    ProviderRegistry, RateLimit, SwitchDecision, CN_PROVIDERS, OVERSEAS_PROVIDERS,
};
pub use providers::{DeepSeekProvider, GenericOpenAiProvider, QwenCloudProvider};
pub use routing::{
    decide_route, write_routing_audit, Route, RouteTarget, RoutingAuditEntry, RoutingAuditSink,
    RoutingDecision,
};
pub use stage::{answer, HsdContext, KbContext, RoutingGuardCtx, RuleContext, StageContext};

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "ai-dispatcher";

/// The dispatcher facade owning the degrade FSM + provider registry + INV-08 templates. The api
/// handler holds one of these (behind its app state) and drives [`Dispatcher::answer`].
pub struct Dispatcher {
    /// The degrade-level state machine.
    pub levels: LevelMachine,
    /// The provider registry (primary/secondary selection).
    pub registry: ProviderRegistry,
    /// The INV-08 templates.
    pub templates: Inv08Templates,
    /// DeepSeek (primary) provider, when configured.
    pub primary: Option<DeepSeekProvider>,
    /// Qwen (secondary) provider, when configured.
    pub secondary: Option<QwenCloudProvider>,
    /// The local small model, when loaded.
    pub local: Option<LocalQwen>,
}

impl Dispatcher {
    /// Build a dispatcher from parsed secrets + a (possibly absent) local model. Both real
    /// providers point at the SiliconFlow gateway.
    pub fn from_secrets(
        secrets: &SecretsConfig,
        local: Option<LocalQwen>,
    ) -> Result<Self, DispatcherError> {
        let primary =
            DeepSeekProvider::from_config(&secrets.siliconflow, &secrets.network.no_proxy)?;
        let secondary =
            QwenCloudProvider::from_config(&secrets.siliconflow, &secrets.network.no_proxy)?;
        Ok(Self {
            levels: LevelMachine::new(),
            registry: ProviderRegistry::default_siliconflow(),
            templates: Inv08Templates::default(),
            primary: Some(primary),
            secondary: Some(secondary),
            local,
        })
    }

    /// The current degrade level.
    pub fn current_level(&self) -> DegradeLevel {
        self.levels.current()
    }

    /// Run a query through the A→F pipeline against the live `hsd` / `kb` / `rule-engine` subsystems.
    /// `rule_request` is the caller-derived intent (None ⇒ Stage D is skipped, AI answers directly).
    ///
    /// `guard` carries the data-export-guard inputs ([`RoutingGuardCtx`]): the user's overseas-cloud
    /// settings flag (`compliance/02` §3.1 `overseas_enabled`; default false) and the live
    /// routing-decision audit sink (`compliance/02` §6, INV-06). On the LIVE path the api layer
    /// ALWAYS supplies a real sink so EVERY data-export-guard decision is written as the four-tuple
    /// `what` (not just blocks). Threading this through here is what makes the §3 export guard run on
    /// the genuine dispatch — never dead code (未通过禁止发出).
    pub async fn answer(
        &self,
        query: &UserQuery,
        hsd: &dyn HsdContext,
        kb: &dyn KbContext,
        rules: &dyn RuleContext,
        rule_request: Option<rule_engine::RuleRequest>,
        guard: RoutingGuardCtx<'_>,
    ) -> Answer {
        let ctx = StageContext {
            hsd,
            kb,
            rules,
            level: self.levels.current(),
            primary_id: self.registry.primary,
            secondary_id: self.registry.secondary,
            primary: self.primary.as_ref().map(|p| p as &dyn Provider),
            secondary: self.secondary.as_ref().map(|p| p as &dyn Provider),
            local: self.local.as_ref(),
            templates: &self.templates,
            rule_request,
            overseas_enabled: guard.overseas_enabled,
            audit: guard.audit,
        };
        stage::answer(query, &ctx).await
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "ai-dispatcher");
    }
}
