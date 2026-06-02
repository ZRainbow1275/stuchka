//! Local small model (Qwen2.5-7B-Q4) fallback (ai/01 §1.4).
//!
//! R1a scope (brief §7): [`LocalQwen::try_load`] is real — a missing weight file returns `Ok(None)`
//! (graceful Level2 skip), a SHA-256 mismatch returns `Err(ModelTampered)`. The Level2 routing
//! branch is present in [`crate::stage`]. The actual llama.cpp / candle inference is **deferred**
//! (deferred_usable): [`LocalQwen::complete`] returns a clearly-labelled not-yet-downloaded notice
//! rather than fabricating a model answer — there is no fake inference on the live path.

use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::config::LocalConfig;
use crate::error::DispatcherError;
use crate::provider::{CompleteRequest, CompleteResponse, ProviderId};

/// A loaded local model handle (ai/01 §1.4). The real backend handle (llama.cpp / candle) is
/// deferred; this round carries the validated path + config so the Level2 branch is wired.
#[derive(Debug, Clone)]
pub struct LocalQwen {
    /// Validated weight path.
    model_path: PathBuf,
    /// Context window.
    ctx_size: u32,
    /// Inference threads.
    threads: u32,
    /// Whether a real inference backend is attached (always false this round → deferred).
    backend_ready: bool,
}

impl LocalQwen {
    /// Attempt to load the local model (ai/01 §1.4):
    /// - weight file absent → `Ok(None)` (user has not downloaded it → Level2 is skipped);
    /// - SHA-256 mismatch (when an `expected_sha` is configured) → `Err(ModelTampered)`;
    /// - present + checksum OK → `Ok(Some(LocalQwen))` with the backend deferred.
    pub fn try_load(cfg: &LocalConfig) -> Result<Option<Self>, DispatcherError> {
        if !cfg.model_path.exists() {
            return Ok(None);
        }
        if !cfg.expected_sha.trim().is_empty() {
            let actual = sha256_file(&cfg.model_path)?;
            if !actual.eq_ignore_ascii_case(cfg.expected_sha.trim()) {
                return Err(DispatcherError::ModelTampered);
            }
        }
        Ok(Some(Self {
            model_path: cfg.model_path.clone(),
            ctx_size: cfg.ctx_size,
            threads: cfg.threads,
            // Real llama.cpp/candle inference is deferred (deferred_usable); no fake backend.
            backend_ready: false,
        }))
    }

    /// The validated weight path.
    pub fn model_path(&self) -> &PathBuf {
        &self.model_path
    }

    /// Context window.
    pub fn ctx_size(&self) -> u32 {
        self.ctx_size
    }

    /// Inference threads.
    pub fn threads(&self) -> u32 {
        self.threads
    }

    /// Whether a real inference backend is attached. Always `false` this round (deferred).
    pub fn is_inference_ready(&self) -> bool {
        self.backend_ready
    }

    /// Run a local completion. The real llama.cpp/candle backend is deferred (deferred_usable);
    /// until it ships this returns an honest "local inference not yet available" notice instead of
    /// fabricating a legal answer (no fake-data on the live path — task honesty gate).
    pub async fn complete(
        &self,
        _req: CompleteRequest,
    ) -> Result<CompleteResponse, DispatcherError> {
        Ok(CompleteResponse {
            content: "本地小模型已就位，但本机推理后端尚未启用（后续版本提供）。当前请改用规则计算、时效提示，或在确认后使用云端服务。"
                .to_string(),
            provider: ProviderId::LocalQwen,
            model: "qwen2.5-7b-q4".to_string(),
            self_reported_confidence: Some(0.3),
            total_tokens: None,
        })
    }
}

/// SHA-256 of a file (lowercase hex). Streams in 64KiB chunks (the weight file is ~5GB).
fn sha256_file(path: &PathBuf) -> Result<String, DispatcherError> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .map_err(|e| DispatcherError::Config(format!("open {}: {e}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| DispatcherError::Config(format!("read {}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_weights_returns_none() {
        let cfg = LocalConfig {
            model_path: PathBuf::from("/no/such/qwen.gguf"),
            expected_sha: String::new(),
            ctx_size: 4096,
            threads: 4,
        };
        let loaded = LocalQwen::try_load(&cfg).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn tampered_weights_error() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("aidisp_qwen_test_{}.gguf", std::process::id()));
        std::fs::write(&path, b"not real weights").unwrap();
        let cfg = LocalConfig {
            model_path: path.clone(),
            expected_sha: "deadbeef".repeat(8), // wrong
            ctx_size: 4096,
            threads: 4,
        };
        let res = LocalQwen::try_load(&cfg);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(res, Err(DispatcherError::ModelTampered)));
    }

    #[test]
    fn present_with_correct_sha_loads() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("aidisp_qwen_ok_{}.gguf", std::process::id()));
        let content = b"dummy weights";
        std::fs::write(&path, content).unwrap();
        let sha = {
            let mut h = Sha256::new();
            h.update(content);
            hex_lower(&h.finalize())
        };
        let cfg = LocalConfig {
            model_path: path.clone(),
            expected_sha: sha,
            ctx_size: 4096,
            threads: 4,
        };
        let res = LocalQwen::try_load(&cfg);
        let _ = std::fs::remove_file(&path);
        let loaded = res.unwrap();
        assert!(loaded.is_some());
        assert!(!loaded.unwrap().is_inference_ready()); // deferred
    }
}
