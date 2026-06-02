//! Provider abstraction + primary/secondary switching (ai/01 §1.3).
//!
//! [`Provider`] is the async trait every cloud / local backend implements. It is object-safe so the
//! registry can hold `Box<dyn Provider>` (hence `#[async_trait]`, not bare AFIT — brief §3.2). The
//! concrete implementations live in [`crate::providers`]; this module owns the contract types
//! ([`ProviderId`], [`CompleteRequest`], [`CompleteResponse`], [`HealthCheck`], [`RateLimit`]),
//! the [`ProviderRegistry`], and the [`SwitchDecision`] policy.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::DispatcherError;

/// Provider identity (ai/01 §1.3). The five 境内 ids are CN-jurisdiction; the three 境外 ids are
/// overseas (default-disabled, need a signature); `LocalQwen` is the offline fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    /// DeepSeek (primary this round, via the SiliconFlow gateway).
    DeepSeek,
    /// 文心 Ernie.
    Ernie,
    /// 通义 Qwen cloud (secondary this round, via the SiliconFlow gateway).
    QwenCloud,
    /// 智谱 Zhipu.
    Zhipu,
    /// Moonshot.
    Moonshot,
    /// Anthropic Claude (overseas).
    Claude,
    /// Google Gemini (overseas).
    Gemini,
    /// OpenAI (overseas).
    OpenAI,
    /// Local Qwen2.5-7B-Q4 small model.
    LocalQwen,
}

impl ProviderId {
    /// CN jurisdiction (no cross-border consent needed; light confirm only).
    pub fn is_cn_jurisdiction(self) -> bool {
        matches!(
            self,
            Self::DeepSeek | Self::Ernie | Self::QwenCloud | Self::Zhipu | Self::Moonshot
        )
    }

    /// Overseas jurisdiction (default disabled; every request needs a fresh signature, C-B-5).
    pub fn is_overseas(self) -> bool {
        matches!(self, Self::Claude | Self::Gemini | Self::OpenAI)
    }

    /// The local offline model (never leaves the device).
    pub fn is_local(self) -> bool {
        matches!(self, Self::LocalQwen)
    }

    /// Stable `&'static str` id used by [`Provider::id`] and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::Ernie => "ernie",
            Self::QwenCloud => "qwen_cloud",
            Self::Zhipu => "zhipu",
            Self::Moonshot => "moonshot",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::OpenAI => "openai",
            Self::LocalQwen => "local_qwen",
        }
    }
}

/// The five 境内 ids offered in the onboarding wizard (ai/01 §1.6 primary/secondary picks).
pub const CN_PROVIDERS: [ProviderId; 5] = [
    ProviderId::DeepSeek,
    ProviderId::Ernie,
    ProviderId::QwenCloud,
    ProviderId::Zhipu,
    ProviderId::Moonshot,
];

/// The three overseas ids (default disabled).
pub const OVERSEAS_PROVIDERS: [ProviderId; 3] =
    [ProviderId::Claude, ProviderId::Gemini, ProviderId::OpenAI];

/// A chat-completion request handed to a [`Provider`] (provider-agnostic; the OpenAI-compatible
/// client maps it onto `POST /chat/completions`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteRequest {
    /// System prompt (legal-assistant role framing).
    pub system: String,
    /// The user prompt (already PII-masked at the call site if needed).
    pub user: String,
    /// Sampling temperature.
    pub temperature: f32,
    /// Max output tokens.
    pub max_tokens: u32,
}

impl CompleteRequest {
    /// A minimal request with the project's default sampling.
    pub fn new(system: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            user: user.into(),
            temperature: 0.2,
            max_tokens: 1024,
        }
    }
}

/// A chat-completion response (provider-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteResponse {
    /// The generated text.
    pub content: String,
    /// Which provider produced it.
    pub provider: ProviderId,
    /// Model id reported by the gateway.
    pub model: String,
    /// Provider self-reported confidence / logprob proxy, when available (INV-08 confidence input).
    pub self_reported_confidence: Option<f32>,
    /// Prompt + completion token usage, when reported.
    pub total_tokens: Option<u32>,
}

/// A health-probe result (`/llm/provider/test`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// The probed provider.
    pub provider: ProviderId,
    /// Whether the gateway answered OK.
    pub healthy: bool,
    /// Round-trip latency in milliseconds.
    pub latency_ms: u64,
    /// Optional gateway-reported model id (confirms the endpoint is live).
    pub model: Option<String>,
}

/// A provider's advertised rate-limit hint (ai/01 §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimit {
    /// Requests per minute.
    pub rpm: u32,
    /// Tokens per minute.
    pub tpm: u32,
}

/// The async provider trait (ai/01 §1.3). Object-safe so the registry can box implementations.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Stable id string.
    fn id(&self) -> &'static str;
    /// Whether the provider is in CN jurisdiction (gates the cross-border confirm dialog).
    fn is_cn_jurisdiction(&self) -> bool;
    /// Probe gateway health (lightweight `GET /models` or a tiny completion).
    async fn health(&self) -> Result<HealthCheck, DispatcherError>;
    /// Run a chat completion.
    async fn complete(&self, req: CompleteRequest) -> Result<CompleteResponse, DispatcherError>;
    /// Advertised rate-limit hint.
    fn rate_limit_hint(&self) -> RateLimit;
}

/// The user's primary/secondary provider selection (ai/01 §1.3). `secondary` is forced `!=
/// primary` by the onboarding validator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRegistry {
    /// Primary provider (one of the five CN ids).
    pub primary: ProviderId,
    /// Secondary provider (forced `!= primary`).
    pub secondary: ProviderId,
    /// Whether overseas providers are enabled (default false).
    pub overseas_enabled: bool,
    /// When overseas use was consented to (signature ts), if ever.
    pub overseas_consented_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ProviderRegistry {
    /// Construct a CN-only registry (overseas disabled).
    pub fn cn_only(primary: ProviderId, secondary: ProviderId) -> Self {
        Self {
            primary,
            secondary,
            overseas_enabled: false,
            overseas_consented_at: None,
        }
    }

    /// The default real-deployment registry this round: DeepSeek primary, Qwen secondary, both via
    /// the SiliconFlow gateway, overseas disabled.
    pub fn default_siliconflow() -> Self {
        Self::cn_only(ProviderId::DeepSeek, ProviderId::QwenCloud)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::default_siliconflow()
    }
}

/// Primary/secondary switching decision (ai/01 §1.3). Returned by [`decide_switch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchDecision {
    /// First failure — retry the primary once.
    RetryPrimaryOnce,
    /// Switch to the secondary (LevelMachine moves to Level1 next tick).
    Failover {
        /// The provider to fail over to.
        to: ProviderId,
    },
    /// Secondary also failed — within the 5-minute cooldown go straight to local.
    CooldownThenLocal,
    /// No AI resource at all — only the M5 / M9 / M16 pure-rule modules.
    Level3RuleOnly,
}

/// Decide the next switching action given how the chain has failed so far.
///
/// `primary_failed` is whether the primary has already errored this round; `attempt` is how many
/// times the primary has been tried (1 = first failure); `secondary_failed` is whether the secondary
/// has also errored; `local_available` is whether the local model can pick up.
pub fn decide_switch(
    registry: &ProviderRegistry,
    primary_failed: bool,
    attempt: u8,
    secondary_failed: bool,
    local_available: bool,
) -> SwitchDecision {
    if primary_failed && attempt < 2 {
        // First failure: retry once before failing over (avoids flapping on a transient blip).
        return SwitchDecision::RetryPrimaryOnce;
    }
    if primary_failed && !secondary_failed {
        return SwitchDecision::Failover {
            to: registry.secondary,
        };
    }
    // Primary + secondary both down.
    if local_available {
        SwitchDecision::CooldownThenLocal
    } else {
        SwitchDecision::Level3RuleOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jurisdiction_classification() {
        for p in CN_PROVIDERS {
            assert!(p.is_cn_jurisdiction());
            assert!(!p.is_overseas());
        }
        for p in OVERSEAS_PROVIDERS {
            assert!(p.is_overseas());
            assert!(!p.is_cn_jurisdiction());
        }
        assert!(ProviderId::LocalQwen.is_local());
    }

    #[test]
    fn switch_retries_then_fails_over() {
        let reg = ProviderRegistry::default_siliconflow();
        assert_eq!(
            decide_switch(&reg, true, 1, false, true),
            SwitchDecision::RetryPrimaryOnce
        );
        assert_eq!(
            decide_switch(&reg, true, 2, false, true),
            SwitchDecision::Failover {
                to: ProviderId::QwenCloud
            }
        );
    }

    #[test]
    fn both_down_goes_local_then_rule() {
        let reg = ProviderRegistry::default_siliconflow();
        assert_eq!(
            decide_switch(&reg, true, 2, true, true),
            SwitchDecision::CooldownThenLocal
        );
        assert_eq!(
            decide_switch(&reg, true, 2, true, false),
            SwitchDecision::Level3RuleOnly
        );
    }

    #[test]
    fn provider_id_serde_snake_case() {
        assert_eq!(
            serde_json::to_string(&ProviderId::QwenCloud).unwrap(),
            "\"qwen_cloud\""
        );
        assert_eq!(
            serde_json::to_string(&ProviderId::LocalQwen).unwrap(),
            "\"local_qwen\""
        );
    }
}
