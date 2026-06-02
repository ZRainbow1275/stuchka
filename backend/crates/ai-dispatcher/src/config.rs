//! Gateway secrets + local-model config loading (父 design §3).
//!
//! Reads `Stučka/backend/config/secrets.toml` (gitignored). The `[siliconflow]` section carries
//! `base_url` / `api_key` / `primary_model` / `secondary_model`; the `[network]` section carries
//! `no_proxy`. The api key is wrapped in [`secrecy::SecretString`] the instant it leaves the file
//! so it can never be `Debug`-printed or serialised. The NO_PROXY value MUST contain
//! `127.0.0.1,localhost,api.siliconflow.cn` (user-environment system proxy 2080 would otherwise
//! break the local IPC + the direct SiliconFlow leg).

use std::path::Path;

use secrecy::SecretString;
use serde::Deserialize;

use crate::error::DispatcherError;

/// Parsed `[siliconflow]` gateway config (the OpenAI-compatible endpoint both DeepSeek-primary and
/// Qwen-secondary point at this round).
#[derive(Clone)]
pub struct SiliconFlowConfig {
    /// e.g. `https://api.siliconflow.cn/v1`.
    pub base_url: String,
    /// The gateway api key — never logged, never serialised (secrecy guard).
    pub api_key: SecretString,
    /// Primary chat model id, e.g. `deepseek-ai/DeepSeek-V3`.
    pub primary_model: String,
    /// Secondary chat model id, e.g. `Qwen/Qwen2.5-7B-Instruct`.
    pub secondary_model: String,
}

// Hand `Debug` so a stray `{:?}` on the config never leaks the key.
impl std::fmt::Debug for SiliconFlowConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SiliconFlowConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("primary_model", &self.primary_model)
            .field("secondary_model", &self.secondary_model)
            .finish()
    }
}

/// Parsed `[network]` section.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    /// Comma-separated NO_PROXY hosts; must include `api.siliconflow.cn` + the loopbacks.
    pub no_proxy: String,
}

impl NetworkConfig {
    /// Whether NO_PROXY covers the SiliconFlow gateway host (hard requirement, 父 design §3).
    pub fn covers_siliconflow(&self) -> bool {
        self.no_proxy.contains("api.siliconflow.cn")
    }

    /// Whether NO_PROXY covers the loopback hosts (local IPC must never go through the system proxy).
    pub fn covers_loopback(&self) -> bool {
        self.no_proxy.contains("127.0.0.1") && self.no_proxy.contains("localhost")
    }
}

/// The whole `secrets.toml`.
#[derive(Debug, Clone)]
pub struct SecretsConfig {
    /// SiliconFlow gateway section.
    pub siliconflow: SiliconFlowConfig,
    /// Network / proxy section.
    pub network: NetworkConfig,
}

// --- serde shadow types (the raw TOML shape; `api_key` is a plain String here, immediately moved
//     into a SecretString in `from_raw` and never retained) -----------------------------------

#[derive(Deserialize)]
struct RawSecrets {
    siliconflow: RawSiliconFlow,
    #[serde(default)]
    network: Option<RawNetwork>,
}

#[derive(Deserialize)]
struct RawSiliconFlow {
    base_url: String,
    api_key: String,
    primary_model: String,
    secondary_model: String,
}

#[derive(Deserialize, Default)]
struct RawNetwork {
    #[serde(default)]
    no_proxy: String,
}

impl SecretsConfig {
    /// Parse a `secrets.toml` string. The default NO_PROXY (when `[network]` is absent) is the
    /// hard-required loopback + gateway set.
    pub fn from_toml_str(s: &str) -> Result<Self, DispatcherError> {
        let raw: RawSecrets = toml::from_str(s)
            .map_err(|e| DispatcherError::Config(format!("secrets.toml parse: {e}")))?;
        let no_proxy = raw
            .network
            .map(|n| n.no_proxy)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "127.0.0.1,localhost,api.siliconflow.cn".to_string());
        Ok(Self {
            siliconflow: SiliconFlowConfig {
                base_url: raw.siliconflow.base_url,
                api_key: SecretString::from(raw.siliconflow.api_key),
                primary_model: raw.siliconflow.primary_model,
                secondary_model: raw.siliconflow.secondary_model,
            },
            network: NetworkConfig { no_proxy },
        })
    }

    /// Load + parse `secrets.toml` from a path (default `config/secrets.toml`).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, DispatcherError> {
        let path = path.as_ref();
        let s = std::fs::read_to_string(path)
            .map_err(|e| DispatcherError::Config(format!("read {}: {e}", path.display())))?;
        Self::from_toml_str(&s)
    }
}

/// Local Qwen small-model config (ai/01 §1.4).
#[derive(Debug, Clone)]
pub struct LocalConfig {
    /// Path to the gguf weights, e.g. `~/.stuchka/models/qwen2.5-7b-instruct-q4_k_m.gguf`.
    pub model_path: std::path::PathBuf,
    /// Expected SHA-256 (lowercase hex) of the weight file.
    pub expected_sha: String,
    /// Context window.
    pub ctx_size: u32,
    /// Inference threads (default num_cpus/2).
    pub threads: u32,
}

impl Default for LocalConfig {
    fn default() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        Self {
            model_path: std::path::Path::new(&home)
                .join(".stuchka")
                .join("models")
                .join("qwen2.5-7b-instruct-q4_k_m.gguf"),
            expected_sha: String::new(),
            ctx_size: 4096,
            threads: (num_cpus::get() as u32 / 2).max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    const SAMPLE: &str = r#"
[siliconflow]
base_url = "https://api.siliconflow.cn/v1"
api_key = "sk-test-do-not-log"
primary_model = "deepseek-ai/DeepSeek-V3"
secondary_model = "Qwen/Qwen2.5-7B-Instruct"

[network]
no_proxy = "127.0.0.1,localhost,api.siliconflow.cn"
"#;

    #[test]
    fn parses_siliconflow_section() {
        let cfg = SecretsConfig::from_toml_str(SAMPLE).unwrap();
        assert_eq!(cfg.siliconflow.base_url, "https://api.siliconflow.cn/v1");
        assert_eq!(cfg.siliconflow.primary_model, "deepseek-ai/DeepSeek-V3");
        assert_eq!(cfg.siliconflow.secondary_model, "Qwen/Qwen2.5-7B-Instruct");
        assert_eq!(
            cfg.siliconflow.api_key.expose_secret(),
            "sk-test-do-not-log"
        );
    }

    #[test]
    fn no_proxy_covers_required_hosts() {
        let cfg = SecretsConfig::from_toml_str(SAMPLE).unwrap();
        assert!(cfg.network.covers_siliconflow());
        assert!(cfg.network.covers_loopback());
    }

    #[test]
    fn debug_never_leaks_key() {
        let cfg = SecretsConfig::from_toml_str(SAMPLE).unwrap();
        let dbg = format!("{:?}", cfg);
        assert!(!dbg.contains("sk-test-do-not-log"), "key leaked: {dbg}");
        assert!(dbg.contains("<redacted>"));
    }

    #[test]
    fn missing_network_defaults_no_proxy() {
        let minimal = r#"
[siliconflow]
base_url = "https://api.siliconflow.cn/v1"
api_key = "k"
primary_model = "a"
secondary_model = "b"
"#;
        let cfg = SecretsConfig::from_toml_str(minimal).unwrap();
        assert!(cfg.network.covers_siliconflow());
        assert!(cfg.network.covers_loopback());
    }
}
