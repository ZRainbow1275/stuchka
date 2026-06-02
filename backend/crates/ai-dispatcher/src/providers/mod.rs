//! Concrete provider implementations (brief §6 I-3: the spec's single `provider.rs` is split into
//! a `providers/` submodule for readability).
//!
//! - [`openai_compat::OpenAiCompatClient`] — the SiliconFlow gateway client both real providers reuse.
//! - [`DeepSeekProvider`] (primary) + [`QwenCloudProvider`] (secondary) — REAL this round, pointing
//!   at the SiliconFlow gateway with the DeepSeek-V3 / Qwen2.5 models from `secrets.toml`.
//! - The remaining 境内 (Ernie / Zhipu / Moonshot) + 境外 (Claude / Gemini / OpenAI) ids keep a
//!   trait skeleton + config placeholder (make-usable, not wired this round — brief §7).

pub mod openai_compat;

use async_trait::async_trait;
use secrecy::SecretString;

use crate::config::SiliconFlowConfig;
use crate::error::DispatcherError;
use crate::provider::{
    CompleteRequest, CompleteResponse, HealthCheck, Provider, ProviderId, RateLimit,
};
use openai_compat::OpenAiCompatClient;

/// DeepSeek provider (primary), reaching DeepSeek-V3 through the SiliconFlow gateway.
#[derive(Debug, Clone)]
pub struct DeepSeekProvider {
    client: OpenAiCompatClient,
}

impl DeepSeekProvider {
    /// Build from the parsed `[siliconflow]` config (uses `primary_model`).
    pub fn from_config(cfg: &SiliconFlowConfig, no_proxy: &str) -> Result<Self, DispatcherError> {
        let client = OpenAiCompatClient::new(
            ProviderId::DeepSeek,
            cfg.base_url.clone(),
            cfg.primary_model.clone(),
            cfg.api_key.clone(),
            no_proxy,
        )?;
        Ok(Self { client })
    }

    /// Construct directly from an explicit base_url + model + key (testing / custom gateways).
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: SecretString,
        no_proxy: &str,
    ) -> Result<Self, DispatcherError> {
        Ok(Self {
            client: OpenAiCompatClient::new(
                ProviderId::DeepSeek,
                base_url,
                model,
                api_key,
                no_proxy,
            )?,
        })
    }
}

#[async_trait]
impl Provider for DeepSeekProvider {
    fn id(&self) -> &'static str {
        ProviderId::DeepSeek.as_str()
    }
    fn is_cn_jurisdiction(&self) -> bool {
        true
    }
    async fn health(&self) -> Result<HealthCheck, DispatcherError> {
        self.client.health().await
    }
    async fn complete(&self, req: CompleteRequest) -> Result<CompleteResponse, DispatcherError> {
        self.client.complete(req).await
    }
    fn rate_limit_hint(&self) -> RateLimit {
        self.client.rate_limit_hint()
    }
}

/// Qwen cloud provider (secondary), reaching Qwen2.5 through the SiliconFlow gateway.
#[derive(Debug, Clone)]
pub struct QwenCloudProvider {
    client: OpenAiCompatClient,
}

impl QwenCloudProvider {
    /// Build from the parsed `[siliconflow]` config (uses `secondary_model`).
    pub fn from_config(cfg: &SiliconFlowConfig, no_proxy: &str) -> Result<Self, DispatcherError> {
        let client = OpenAiCompatClient::new(
            ProviderId::QwenCloud,
            cfg.base_url.clone(),
            cfg.secondary_model.clone(),
            cfg.api_key.clone(),
            no_proxy,
        )?;
        Ok(Self { client })
    }

    /// Construct directly from explicit parameters.
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: SecretString,
        no_proxy: &str,
    ) -> Result<Self, DispatcherError> {
        Ok(Self {
            client: OpenAiCompatClient::new(
                ProviderId::QwenCloud,
                base_url,
                model,
                api_key,
                no_proxy,
            )?,
        })
    }
}

#[async_trait]
impl Provider for QwenCloudProvider {
    fn id(&self) -> &'static str {
        ProviderId::QwenCloud.as_str()
    }
    fn is_cn_jurisdiction(&self) -> bool {
        true
    }
    async fn health(&self) -> Result<HealthCheck, DispatcherError> {
        self.client.health().await
    }
    async fn complete(&self, req: CompleteRequest) -> Result<CompleteResponse, DispatcherError> {
        self.client.complete(req).await
    }
    fn rate_limit_hint(&self) -> RateLimit {
        self.client.rate_limit_hint()
    }
}

/// Make-usable skeleton for the remaining cloud providers (Ernie / Zhipu / Moonshot / Claude /
/// Gemini / OpenAI). Holds an OpenAI-compatible client so it is config-ready, but is NOT wired into
/// the default registry this round (brief §7). Overseas ids additionally need a signature before
/// any request leaves the device (enforced by the confirm protocol).
#[derive(Debug, Clone)]
pub struct GenericOpenAiProvider {
    id: ProviderId,
    client: OpenAiCompatClient,
}

impl GenericOpenAiProvider {
    /// Build a generic provider for any `id` against an OpenAI-compatible `base_url`.
    pub fn new(
        id: ProviderId,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: SecretString,
        no_proxy: &str,
    ) -> Result<Self, DispatcherError> {
        Ok(Self {
            id,
            client: OpenAiCompatClient::new(id, base_url, model, api_key, no_proxy)?,
        })
    }
}

#[async_trait]
impl Provider for GenericOpenAiProvider {
    fn id(&self) -> &'static str {
        self.id.as_str()
    }
    fn is_cn_jurisdiction(&self) -> bool {
        self.id.is_cn_jurisdiction()
    }
    async fn health(&self) -> Result<HealthCheck, DispatcherError> {
        self.client.health().await
    }
    async fn complete(&self, req: CompleteRequest) -> Result<CompleteResponse, DispatcherError> {
        self.client.complete(req).await
    }
    fn rate_limit_hint(&self) -> RateLimit {
        self.client.rate_limit_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepseek_is_cn_and_named() {
        let p = DeepSeekProvider::new(
            "https://api.siliconflow.cn/v1",
            "deepseek-ai/DeepSeek-V3",
            SecretString::from("k".to_string()),
            "127.0.0.1,localhost,api.siliconflow.cn",
        )
        .unwrap();
        assert_eq!(p.id(), "deepseek");
        assert!(p.is_cn_jurisdiction());
    }

    #[test]
    fn qwen_is_cn_and_named() {
        let p = QwenCloudProvider::new(
            "https://api.siliconflow.cn/v1",
            "Qwen/Qwen2.5-7B-Instruct",
            SecretString::from("k".to_string()),
            "127.0.0.1,localhost,api.siliconflow.cn",
        )
        .unwrap();
        assert_eq!(p.id(), "qwen_cloud");
        assert!(p.is_cn_jurisdiction());
    }

    #[test]
    fn overseas_generic_not_cn() {
        let p = GenericOpenAiProvider::new(
            ProviderId::OpenAI,
            "https://api.openai.com/v1",
            "gpt-4o",
            SecretString::from("k".to_string()),
            "127.0.0.1,localhost",
        )
        .unwrap();
        assert!(!p.is_cn_jurisdiction());
        assert_eq!(p.id(), "openai");
    }
}
