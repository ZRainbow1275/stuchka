//! `stuchka-core` binary — the forked Rust child process (D1 · backend/01 §1.1.1).
//!
//! Sequence: init tracing → `core::Boot::prepare()` (load config, bind `127.0.0.1:0`, generate
//! token) → print the single `READY{"port":N,"token":"<hex>"}` line to stdout → build the axum
//! router (all routes + WS, fronted by the Bearer middleware) → `axum::serve`.
//!
//! The READY line is printed exactly once, flushed, before serving, so the Flutter parent can
//! parse it and build its HTTP client (backend/01 §1.1.2 Dart contract).

use std::io::Write;

use api::{router, AppState};
use stuchka_core::{Boot, BootServices};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    // Boot: config (NO_PROXY injected) + loopback bind + 64-byte token.
    let boot = Boot::prepare().await?;
    let token = boot.handshake.token.clone();

    // Initialize the real R1a module services (db + audit + DEK + rule-engine + KB + hsd + age).
    // This runs migrations, verifies the audit chain on startup, and derives the session DEK via
    // the real Argon2/HKDF path — all before READY so the parent only sees a ready backend.
    let services = BootServices::initialize(&boot.config).await?;

    // Announce READY on stdout, exactly one line, flushed before serving.
    emit_ready(&boot.ready_line())?;

    // Build the fully-wired app (data/compute + AI dispatcher from config/secrets.toml + live Yjs
    // sync provider) from the boot token + services and serve on the already-bound listener.
    let state = AppState::from_boot_wired(token, services, "config/secrets.toml").await?;
    let app = router(state);
    let listener = boot.listener;
    tracing::info!(
        port = boot.handshake.port,
        "stuchka-core serving on loopback"
    );
    axum::serve(listener, app).await?;
    Ok(())
}

/// Print the READY line to stdout and flush (no buffering surprises for the parent).
fn emit_ready(line: &str) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    writeln!(lock, "{line}")?;
    lock.flush()
}

/// JSON structured logging to stderr (stdout is reserved for the READY line; §5.5.1).
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .json()
        .try_init();
}
