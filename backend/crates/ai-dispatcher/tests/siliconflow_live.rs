//! REAL SiliconFlow gateway integration (brief acceptance #2).
//!
//! Gated behind `#[ignore]` so a network outage NEVER fails the crate's `cargo test`. Run it
//! explicitly: `cargo test -p ai-dispatcher -- --ignored`. It reads `config/secrets.toml`
//! (gitignored; the key is there and is never printed), builds the DeepSeek-primary +
//! Qwen-secondary providers against the SiliconFlow OpenAI-compatible gateway, and performs a real
//! `health()` + `complete()`. The api key is held in a `SecretString` throughout — only the prompt
//! and a short prefix of the answer are ever printed.

use std::path::PathBuf;

use ai_dispatcher::{
    CompleteRequest, DeepSeekProvider, Provider, QwenCloudProvider, SecretsConfig,
};

/// Resolve `config/secrets.toml` relative to the workspace root (the crate runs from its own dir).
fn secrets_path() -> PathBuf {
    // crates/ai-dispatcher → ../../config/secrets.toml
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent() // crates/
        .and_then(|p| p.parent()) // backend/
        .map(|p| p.join("config").join("secrets.toml"))
        .expect("workspace root resolvable")
}

#[tokio::test]
#[ignore = "network: real SiliconFlow gateway; run with --ignored"]
async fn siliconflow_health_and_chat_completion() {
    let path = secrets_path();
    let secrets =
        SecretsConfig::load(&path).expect("config/secrets.toml must exist for the live test");

    // Hard NO_PROXY requirement: the gateway host + loopbacks must bypass the system proxy.
    assert!(
        secrets.network.covers_siliconflow(),
        "NO_PROXY must include api.siliconflow.cn"
    );

    let no_proxy = &secrets.network.no_proxy;
    let primary = DeepSeekProvider::from_config(&secrets.siliconflow, no_proxy)
        .expect("build DeepSeek provider");
    let secondary = QwenCloudProvider::from_config(&secrets.siliconflow, no_proxy)
        .expect("build Qwen provider");

    // 1) Real health probe (GET /models).
    let health = primary.health().await;
    match &health {
        Ok(h) => println!(
            "[live] DeepSeek health OK: healthy={} latency_ms={} model={:?}",
            h.healthy, h.latency_ms, h.model
        ),
        Err(e) => println!("[live] DeepSeek health error: {e}"),
    }
    let health = health.expect("DeepSeek health must succeed against the live gateway");
    assert!(health.healthy);

    // 2) Real chat completion (POST /chat/completions).
    let req = CompleteRequest::new(
        "你是劳动法信息助理，只提供客观法律信息。",
        "用一句话说明：劳动合同法中经济补偿的计算依据的法条编号是多少？",
    );
    let resp = primary.complete(req).await;
    match &resp {
        Ok(r) => {
            let prefix: String = r.content.chars().take(40).collect();
            println!(
                "[live] DeepSeek completion OK: model={} tokens={:?} content_prefix={}",
                r.model, r.total_tokens, prefix
            );
        }
        Err(e) => println!("[live] DeepSeek completion error: {e}"),
    }
    let resp = resp.expect("DeepSeek completion must return content");
    assert!(
        !resp.content.trim().is_empty(),
        "completion must be non-empty"
    );

    // 3) Secondary provider health (Qwen via the same gateway).
    match secondary.health().await {
        Ok(h) => println!("[live] Qwen health OK: latency_ms={}", h.latency_ms),
        Err(e) => println!("[live] Qwen health error: {e}"),
    }
}
