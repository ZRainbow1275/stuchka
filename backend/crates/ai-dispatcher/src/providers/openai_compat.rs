//! OpenAI-compatible client (the SiliconFlow gateway leg, 父 design §3).
//!
//! Both the DeepSeek-primary and Qwen-secondary providers point at the SiliconFlow OpenAI-
//! compatible gateway (`https://api.siliconflow.cn/v1`) via `POST /chat/completions` and
//! `GET /models`. One [`reqwest::Client`] is reused (connection pool) with `rustls-tls` (no system
//! OpenSSL → single-file distribution). NO_PROXY must include `api.siliconflow.cn` + the loopbacks;
//! the builder calls [`reqwest::ClientBuilder::no_proxy`] so the user-environment system proxy 2080
//! never intercepts the direct gateway leg.
//!
//! The api key lives in a [`secrecy::SecretString`]: it is only `.expose_secret()`-ed into the
//! `Authorization` header at request time and never logged.

use std::time::{Duration, Instant};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::error::DispatcherError;
use crate::provider::{CompleteRequest, CompleteResponse, HealthCheck, ProviderId, RateLimit};

/// Hard timeout for a single cloud completion request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Lightweight health-probe timeout.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);

/// A reusable OpenAI-compatible HTTP client bound to one gateway + model.
#[derive(Clone)]
pub struct OpenAiCompatClient {
    /// The owning provider id (DeepSeek / QwenCloud this round).
    provider: ProviderId,
    /// Base url, e.g. `https://api.siliconflow.cn/v1`.
    base_url: String,
    /// Model id, e.g. `deepseek-ai/DeepSeek-V3`.
    model: String,
    /// Bearer token — exposed only into the auth header.
    api_key: SecretString,
    /// Shared reqwest client (connection pool).
    http: reqwest::Client,
}

impl std::fmt::Debug for OpenAiCompatClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCompatClient")
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl OpenAiCompatClient {
    /// Build a client. The `no_proxy` string (from `secrets.toml [network]`) is applied so the
    /// direct gateway + loopback legs bypass the system proxy. Construction fails only if reqwest
    /// cannot build the TLS backend.
    pub fn new(
        provider: ProviderId,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: SecretString,
        no_proxy: &str,
    ) -> Result<Self, DispatcherError> {
        let mut builder = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent("stuchka-ai-dispatcher");

        // If NO_PROXY covers the gateway host, drop the proxy entirely for this client (the gateway
        // is reached directly; loopback IPC also bypasses the proxy). This is the conservative,
        // deterministic choice for the single direct leg this client owns.
        if no_proxy.contains("api.siliconflow.cn") {
            builder = builder.no_proxy();
        }

        let http = builder
            .build()
            .map_err(|e| DispatcherError::Config(format!("reqwest client build: {e}")))?;

        Ok(Self {
            provider,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            api_key,
            http,
        })
    }

    /// The model id this client targets.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The provider id this client speaks for.
    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    /// `GET {base_url}/models` health probe.
    pub async fn health(&self) -> Result<HealthCheck, DispatcherError> {
        let url = format!("{}/models", self.base_url);
        let started = Instant::now();
        let resp = self
            .http
            .get(&url)
            .bearer_auth(self.api_key.expose_secret())
            .timeout(HEALTH_TIMEOUT)
            .send()
            .await
            .map_err(|e| DispatcherError::Transport(self.provider, e.to_string()))?;

        let latency_ms = started.elapsed().as_millis() as u64;
        let status = resp.status();
        if status.as_u16() == 429 {
            return Err(DispatcherError::RateLimited(self.provider));
        }
        if !status.is_success() {
            return Err(DispatcherError::Upstream(self.provider, status.as_u16()));
        }
        Ok(HealthCheck {
            provider: self.provider,
            healthy: true,
            latency_ms,
            model: Some(self.model.clone()),
        })
    }

    /// `POST {base_url}/chat/completions`.
    pub async fn complete(
        &self,
        req: CompleteRequest,
    ) -> Result<CompleteResponse, DispatcherError> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = ChatCompletionReq {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: &req.system,
                },
                ChatMessage {
                    role: "user",
                    content: &req.user,
                },
            ],
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            stream: false,
        };

        let resp = self
            .http
            .post(&url)
            .bearer_auth(self.api_key.expose_secret())
            .json(&body)
            .send()
            .await
            .map_err(|e| DispatcherError::Transport(self.provider, e.to_string()))?;

        let status = resp.status();
        if status.as_u16() == 429 {
            return Err(DispatcherError::RateLimited(self.provider));
        }
        if !status.is_success() {
            return Err(DispatcherError::Upstream(self.provider, status.as_u16()));
        }

        let parsed: ChatCompletionResp = resp
            .json()
            .await
            .map_err(|e| DispatcherError::Transport(self.provider, e.to_string()))?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();

        Ok(CompleteResponse {
            content,
            provider: self.provider,
            model: parsed.model.unwrap_or_else(|| self.model.clone()),
            self_reported_confidence: None,
            total_tokens: parsed.usage.map(|u| u.total_tokens),
        })
    }

    /// Default rate-limit hint for the SiliconFlow gateway (conservative; tuned in R1.5).
    pub fn rate_limit_hint(&self) -> RateLimit {
        RateLimit {
            rpm: 1_000,
            tpm: 50_000,
        }
    }
}

// --- wire types --------------------------------------------------------------------------------

#[derive(Serialize)]
struct ChatCompletionReq<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatCompletionResp {
    #[serde(default)]
    model: Option<String>,
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: RespMessage,
}

#[derive(Deserialize)]
struct RespMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> OpenAiCompatClient {
        OpenAiCompatClient::new(
            ProviderId::DeepSeek,
            "https://api.siliconflow.cn/v1/",
            "deepseek-ai/DeepSeek-V3",
            SecretString::from("k".to_string()),
            "127.0.0.1,localhost,api.siliconflow.cn",
        )
        .unwrap()
    }

    #[test]
    fn base_url_trailing_slash_trimmed() {
        let c = client();
        assert_eq!(c.base_url, "https://api.siliconflow.cn/v1");
    }

    #[test]
    fn debug_redacts_key() {
        let c = client();
        let dbg = format!("{c:?}");
        assert!(dbg.contains("<redacted>"));
        assert!(!dbg.contains("\"k\""));
    }

    #[test]
    fn parses_chat_completion_payload() {
        let raw = r#"{
            "model": "deepseek-ai/DeepSeek-V3",
            "choices": [{"message": {"role": "assistant", "content": "你好"}}],
            "usage": {"total_tokens": 42}
        }"#;
        let parsed: ChatCompletionResp = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.choices[0].message.content, "你好");
        assert_eq!(parsed.usage.unwrap().total_tokens, 42);
    }
}
