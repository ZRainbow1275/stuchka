//! `DispatcherError` + recovery strategy (ai/01 §1.3).
//!
//! The spec places this type in `provider.rs`; per the implementation brief §6 I-3 it is hoisted
//! into its own module. Maps onto the data-model error-code table at the api boundary
//! (`E_PII_BLOCKED` / `E_KB_OUTDATED` / `E_LLM_ABSTENTION` / `E_LLM_PROVIDER_DOWN`).

use data_model::ErrorCode;

use crate::provider::ProviderId;

/// Dispatcher-layer failure (ai/01 §1.3). Every variant maps to one of the data-model
/// error codes via [`DispatcherError::error_code`] so the api layer can build the
/// `ApiEnvelope` without re-classifying.
#[derive(Debug, thiserror::Error)]
pub enum DispatcherError {
    /// Provider health probe failed.
    #[error("provider {0:?} 健康检查失败")]
    HealthFailed(ProviderId),
    /// Provider returned 429 (rate limited).
    #[error("provider {0:?} 限流（429）")]
    RateLimited(ProviderId),
    /// Provider returned a 5xx (or other) server error with the HTTP status.
    #[error("provider {0:?} 服务端错误：{1}")]
    Upstream(ProviderId, u16),
    /// The local model file failed its SHA-256 integrity check.
    #[error("本地模型文件被篡改（SHA-256 不符）")]
    ModelTampered,
    /// Local inference exceeded the 30s hard timeout.
    #[error("本地推理超时（>30s）")]
    LocalTimeout,
    /// High-sensitivity hit but no local model is available — processing stopped (INV-05).
    #[error("高敏命中且无本地模型可用，已停止处理")]
    HsdBlockedNoLocal,
    /// Knowledge base older than 30 days — compensation calculation refused (Level4).
    #[error("知识库过期 > 30 天，赔偿计算已拒绝（Level4）")]
    KbStaleLevel4,
    /// All providers (primary + secondary) are unavailable.
    #[error("所有 AI 服务提供方均不可用")]
    AllProvidersDown,
    /// Transport / I/O error talking to the provider gateway.
    #[error("provider {0:?} 网络错误：{1}")]
    Transport(ProviderId, String),
    /// Configuration error (missing / malformed secrets.toml).
    #[error("配置错误：{0}")]
    Config(String),
}

impl DispatcherError {
    /// Project onto the authoritative data-model error code (backend/01 §1.12).
    pub fn error_code(&self) -> ErrorCode {
        match self {
            DispatcherError::HealthFailed(_)
            | DispatcherError::RateLimited(_)
            | DispatcherError::Upstream(_, _)
            | DispatcherError::Transport(_, _)
            | DispatcherError::AllProvidersDown => ErrorCode::LlmProviderDown,
            DispatcherError::KbStaleLevel4 => ErrorCode::KbOutdated,
            DispatcherError::HsdBlockedNoLocal => ErrorCode::PiiBlocked,
            DispatcherError::ModelTampered
            | DispatcherError::LocalTimeout
            | DispatcherError::Config(_) => ErrorCode::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_failures_map_to_provider_down() {
        assert_eq!(
            DispatcherError::AllProvidersDown.error_code(),
            ErrorCode::LlmProviderDown
        );
        assert_eq!(
            DispatcherError::RateLimited(ProviderId::DeepSeek).error_code(),
            ErrorCode::LlmProviderDown
        );
    }

    #[test]
    fn kb_and_hsd_map_correctly() {
        assert_eq!(
            DispatcherError::KbStaleLevel4.error_code(),
            ErrorCode::KbOutdated
        );
        assert_eq!(
            DispatcherError::HsdBlockedNoLocal.error_code(),
            ErrorCode::PiiBlocked
        );
    }
}
