//! Boot sequence orchestration (D1 · `crates/core`).
//!
//! Two layers:
//! - [`Boot`] ties config load → loopback bind → handshake into one value that `crates/api`'s
//!   `main.rs` consumes for the READY contract (kept here because `core` owns the D1 process
//!   contract).
//! - [`BootServices`] performs the real R1a service initialization: open the main store (run
//!   migrations), open the independent `audit.sqlite` (run its migrations + verify the hash chain
//!   on startup, D3 / INV-06), derive the session DEK via the real Argon2/HKDF create-or-unlock
//!   path (never a fake key), derive the audit key via `crypto::hkdf_subkey`, load
//!   [`rule_engine::RuleEngine`], build the KB BM25 index + manifest from the sample corpus, build
//!   the high-sensitivity detector, and build the age blob store. `crates/api` re-assembles these
//!   handles into its `AppState`.

use std::sync::Arc;

use chrono::Utc;
use tokio::net::TcpListener;

use crate::config::RuntimeConfig;
use crate::first_run::FirstRunWizard;
use crate::spawn::{bind_loopback, Handshake};

use audit::AuditLog;
use crypto::{derive_kek, master_key, AgeStore, EncryptedDek, FileKeystore, Keystore, SessionDek};
use db::Store;
use hsd::HsdDetector;
use kb::{Bm25Index, KbManifest, KbVersion, CATEGORY_TOTAL, SUBCATEGORY_TOTAL};
use rand::RngCore;
use rule_engine::RuleEngine;

/// A fully prepared boot: loaded config, a bound loopback listener, and the handshake
/// (port + token) to announce on stdout.
pub struct Boot {
    /// Loaded + normalized runtime configuration (NO_PROXY already injected).
    pub config: RuntimeConfig,
    /// Listener bound to `127.0.0.1:0` (concrete OS-assigned port).
    pub listener: TcpListener,
    /// READY handshake (port + 64-byte hex token).
    pub handshake: Handshake,
    /// First-run wizard schema placeholder (populated when `config.first_run`).
    pub first_run: Option<FirstRunWizard>,
}

impl Boot {
    /// Run the boot sequence: load config, bind loopback, generate token, build the handshake.
    ///
    /// Does NOT print the READY line or start serving — the caller (`api::main`) does that so
    /// it controls stdout flushing and axum wiring.
    pub async fn prepare() -> std::io::Result<Self> {
        let config = RuntimeConfig::load();
        let listener = bind_loopback().await?;
        let handshake = Handshake::new(&listener)?;
        let first_run = config.first_run.then(FirstRunWizard::pending);
        Ok(Self {
            config,
            listener,
            handshake,
            first_run,
        })
    }

    /// The exact READY line to print to stdout (single line, `READY` prefix + compact JSON).
    pub fn ready_line(&self) -> String {
        self.handshake.ready_line()
    }
}

/// Default dev master password used only when neither `STUCHKA_MASTER_PASSWORD` nor a config key
/// is supplied (R1a headless/dev boot). It still drives the REAL Argon2id KEK derivation and a
/// real random DEK that is persisted as an `EncryptedDek`; it is never a hard-coded key.
const DEV_MASTER_PASSWORD: &str = "stuchka-dev-master";

/// The live R1a service handles produced by [`BootServices::initialize`]. `crates/api` wraps these
/// in its `AppState`. The session DEK ([`crypto::SessionDek`]) is held only here / in `AppState`
/// and never serialized (INV-05 / C-7).
pub struct BootServices {
    /// Main store (sqlx SqlitePool + repos), migrations already applied.
    pub store: Store,
    /// Independent audit log over `audit.sqlite` (D3); `audit_chain_ok` records the startup
    /// verification result.
    pub audit: AuditLog,
    /// `true` when the startup hash-chain verification passed; `false` → writes must be refused
    /// and the API maps to `E_AUDIT_CHAIN_BROKEN`.
    pub audit_chain_ok: bool,
    /// Session-resident data-encryption key (zeroizing; never serialized).
    pub session_dek: SessionDek,
    /// Pure rule engine (M9 compute + M5 deadline), all 31 provinces loaded.
    pub rule_engine: RuleEngine,
    /// High-sensitivity detector (R1a regex strong-signal layer).
    pub hsd: Arc<HsdDetector>,
    /// KB BM25 index seeded with the sample law corpus.
    pub kb_index: Arc<Bm25Index>,
    /// Current KB manifest (the INV-04 freeze anchor source).
    pub kb_manifest: KbManifest,
    /// Current KB version row (derived from the manifest, marked active).
    pub kb_version: KbVersion,
    /// age blob store for high-sensitivity evidence (identity derived from the DEK).
    pub age_store: Arc<AgeStore>,
}

impl BootServices {
    /// Initialize every live service from `config` (D1 boot sequence).
    ///
    /// Order matches the spec: config + secrets → open main db + migrations → open audit.sqlite +
    /// migrations + verify_on_startup → derive session DEK (create-or-unlock) → derive audit key →
    /// load RuleEngine → build KB index + manifest → construct the handles.
    pub async fn initialize(config: &RuntimeConfig) -> anyhow::Result<Self> {
        // Ensure the data directory tree exists.
        tokio::fs::create_dir_all(&config.data_dir).await?;

        // 1. Main store + migrations.
        let store = Store::open_file(config.main_db_path())
            .await
            .map_err(|e| anyhow::anyhow!("open main store: {e}"))?;

        // 2. Session DEK via the REAL Argon2/HKDF create-or-unlock path.
        let session_dek = open_or_create_dek(config)?;

        // 3. Audit key = HKDF(DEK, AUDIT_INFO); open audit.sqlite + migrations.
        let audit_key = audit::derive_audit_key(session_dek.expose());
        let audit = audit::open_audit_log(config.audit_db_path(), &audit_key)
            .await
            .map_err(|e| anyhow::anyhow!("open audit.sqlite: {e}"))?;

        // 4. Startup chain verification (D3 / INV-06). A broken chain does not abort boot; it is
        //    recorded so the API refuses new audit writes and maps to E_AUDIT_CHAIN_BROKEN.
        let audit_chain_ok = match audit.verify_on_startup().await {
            Ok(checked) => {
                tracing::info!(checked, "audit chain verified on startup");
                true
            }
            Err(e) => {
                tracing::error!(error = %e, "audit chain broken on startup; refusing audit writes");
                false
            }
        };

        // 5. Rule engine (all 31 provinces).
        let rule_engine =
            RuleEngine::new().map_err(|e| anyhow::anyhow!("load rule engine: {e}"))?;

        // 6. KB index + manifest from the seeded sample corpus.
        let kb_index =
            Bm25Index::with_sample_data().map_err(|e| anyhow::anyhow!("build kb index: {e}"))?;
        let kb_manifest = sample_manifest();
        let mut kb_version = KbVersion::from_manifest(&kb_manifest, Utc::now());
        kb_version.is_active = true;

        // 7. HSD (R1a regex strong-signal layer) + age blob store.
        let hsd = HsdDetector::new_regex_only();
        let age_store = AgeStore::from_dek(&session_dek, config.blob_dir())
            .map_err(|e| anyhow::anyhow!("build age store: {e}"))?;

        Ok(Self {
            store,
            audit,
            audit_chain_ok,
            session_dek,
            rule_engine,
            hsd: Arc::new(hsd),
            kb_index: Arc::new(kb_index),
            kb_manifest,
            kb_version,
            age_store: Arc::new(age_store),
        })
    }

    /// Initialize a fully in-memory service set for tests (no on-disk files except the age blob
    /// dir, which is created lazily). The DEK is freshly generated; the audit chain starts empty
    /// and therefore verifies clean.
    pub async fn initialize_in_memory(blob_dir: std::path::PathBuf) -> anyhow::Result<Self> {
        let store = Store::open_in_memory()
            .await
            .map_err(|e| anyhow::anyhow!("open in-memory store: {e}"))?;

        let mut dek_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut dek_bytes);
        let session_dek = SessionDek::new(dek_bytes);

        let audit_key = audit::derive_audit_key(session_dek.expose());
        let audit = audit::open_in_memory(&audit_key)
            .await
            .map_err(|e| anyhow::anyhow!("open in-memory audit: {e}"))?;
        let audit_chain_ok = audit.verify_on_startup().await.is_ok();

        let rule_engine =
            RuleEngine::new().map_err(|e| anyhow::anyhow!("load rule engine: {e}"))?;
        let kb_index =
            Bm25Index::with_sample_data().map_err(|e| anyhow::anyhow!("build kb index: {e}"))?;
        let kb_manifest = sample_manifest();
        let mut kb_version = KbVersion::from_manifest(&kb_manifest, Utc::now());
        kb_version.is_active = true;

        let hsd = HsdDetector::new_regex_only();
        let age_store = AgeStore::from_dek(&session_dek, blob_dir)
            .map_err(|e| anyhow::anyhow!("build age store: {e}"))?;

        Ok(Self {
            store,
            audit,
            audit_chain_ok,
            session_dek,
            rule_engine,
            hsd: Arc::new(hsd),
            kb_index: Arc::new(kb_index),
            kb_manifest,
            kb_version,
            age_store: Arc::new(age_store),
        })
    }
}

/// Open the persisted wrapped DEK (unlock) or, on first run, generate a real random DEK and persist
/// it as an `EncryptedDek` under `<data_dir>/keystore/dek.enc` via the real Argon2id KEK + ChaCha20
/// wrap path. Never returns a fake / all-zero key.
fn open_or_create_dek(config: &RuntimeConfig) -> anyhow::Result<SessionDek> {
    let password = config
        .master_password
        .clone()
        .unwrap_or_else(|| DEV_MASTER_PASSWORD.to_string());
    let keystore = FileKeystore::new(config.keystore_path());

    if keystore.exists() {
        let wrapped: EncryptedDek = keystore
            .read_dek()
            .map_err(|e| anyhow::anyhow!("read keystore: {e}"))?;
        let dek = master_key::open_session(&wrapped, &password)
            .map_err(|e| anyhow::anyhow!("unlock session: {e}"))?;
        tracing::info!("unlocked existing keystore DEK");
        Ok(dek)
    } else {
        // First-run: real random salt + DEK, derive the KEK with Argon2id, wrap + persist.
        let mut salt = [0u8; 16];
        let mut dek_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut salt);
        rand::thread_rng().fill_bytes(&mut dek_bytes);
        let kek = derive_kek(&password, &salt).map_err(|e| anyhow::anyhow!("derive kek: {e}"))?;
        let wrapped = master_key::wrap_dek_with_salt(&kek, &dek_bytes, salt)
            .map_err(|e| anyhow::anyhow!("wrap dek: {e}"))?;
        keystore
            .write_dek(&wrapped)
            .map_err(|e| anyhow::anyhow!("persist keystore: {e}"))?;
        tracing::info!("created and persisted a fresh keystore DEK (first run)");
        Ok(SessionDek::new(dek_bytes))
    }
}

/// Build the `KbManifest` for the seeded sample corpus: the deterministic `global_hash` aggregate
/// over the sample clauses (the INV-04 freeze anchor; KBC-02). `generated_at` is `Utc::now()` so a
/// freshly-booted dev KB is `Fresh` (the freshness gate only refuses on >= 30 days).
fn sample_manifest() -> KbManifest {
    let hashes = kb::sample_data::sample_content_hashes();
    let global_hash = kb::compute_global_hash(hashes.iter().map(|(p, h)| (p.clone(), h.clone())));
    KbManifest {
        version_label: format!("dev-{}", Utc::now().format("%Y-%m-%d")),
        generated_at: Utc::now(),
        global_hash,
        schema_version: 1,
        file_count: hashes.len() as i32,
        law_count_national: hashes.len() as i32,
        law_count_local: 0,
        category_total: CATEGORY_TOTAL,
        subcategory_total: SUBCATEGORY_TOTAL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn prepare_binds_loopback_and_builds_handshake() {
        let boot = Boot::prepare().await.unwrap();
        let addr = boot.listener.local_addr().unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(boot.handshake.port, addr.port());
        assert_eq!(boot.handshake.token.len(), 128);
        let line = boot.ready_line();
        assert!(line.starts_with("READY{"), "got {line}");
    }

    #[tokio::test]
    async fn in_memory_services_initialize_and_kb_is_fresh() {
        let dir = std::env::temp_dir().join(format!("stuchka-boot-{}", uuid::Uuid::now_v7()));
        let svc = BootServices::initialize_in_memory(dir.clone())
            .await
            .unwrap();
        assert!(svc.audit_chain_ok, "empty audit chain verifies clean");
        assert_eq!(svc.rule_engine.regions().len(), 31);
        assert!(svc.kb_manifest.validate().is_ok());
        assert!(svc.kb_version.is_active);
        // KB freshly generated → not expired.
        assert!(
            kb::ensure_calculable(svc.kb_manifest.generated_at, Utc::now()).is_ok(),
            "freshly booted KB must be calculable"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn manifest_global_hash_is_deterministic_64_hex() {
        let m1 = sample_manifest();
        let m2 = sample_manifest();
        assert_eq!(m1.global_hash, m2.global_hash);
        assert_eq!(m1.global_hash.len(), 64);
    }
}
