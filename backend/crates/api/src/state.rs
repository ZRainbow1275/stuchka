//! Shared application state (D1 · backend/01 §1.1.3).
//!
//! Holds the handshake Bearer token (constant-time verified by the middleware) plus every live
//! R1a module handle the wired handlers call: the db [`Store`], the independent audit
//! [`AuditLog`] (+ its startup chain-verification flag, D3 / INV-06), the pure
//! [`RuleEngine`] (M9 + M5), the KB BM25 index + current [`KbManifest`] / [`KbVersion`], the
//! high-sensitivity detector [`HsdDetector`], and the crypto session: the [`SessionDek`] for
//! evidence age-encryption and the derived audit key. The DEK is held behind an `Arc` and is
//! never serialized (INV-05 / C-7).

use std::sync::Arc;

use ai_dispatcher::{Dispatcher, SecretsConfig};
use audit::AuditLog;
use crypto::{AgeStore, SessionDek};
use db::Store;
use hsd::HsdDetector;
use kb::{Bm25Index, KbManifest, KbVersion};
use rule_engine::{DiagnosisEngine, RuleEngine};
use subtle::ConstantTimeEq;
use sync::{ensure_schema, SqliteDocMetaRepo, SqliteDocRepo, SqliteDocStepRepo, SqlitePeerRepo};
use uuid::Uuid;

use stuchka_core::BootServices;

use crate::sync_state::SyncState;

/// Cheaply-cloneable shared state injected into every handler and the Bearer middleware.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    /// The handshake token bytes; compared in constant time (subtle) to defeat timing oracles.
    token: Vec<u8>,
    /// Main-store handle (None in the bare token-only skeleton / auth unit tests).
    services: Option<Services>,
}

/// The live module handles (present once boot initialization has run).
struct Services {
    store: Store,
    audit: AuditLog,
    audit_chain_ok: bool,
    rule_engine: RuleEngine,
    diagnosis_engine: DiagnosisEngine,
    hsd: Arc<HsdDetector>,
    kb_index: Arc<Bm25Index>,
    kb_manifest: KbManifest,
    kb_version: KbVersion,
    age_store: Arc<AgeStore>,
    /// Session DEK — held here so evidence age-encryption / re-derivation works; never serialized.
    #[allow(dead_code)]
    session_dek: SessionDek,
    /// The M7 AI dispatcher (constructed from `config/secrets.toml [siliconflow]` + the live
    /// hsd / kb / rule-engine for the three-stage pipeline). `None` when secrets are absent /
    /// unparseable — `/llm/query` then maps to `E_LLM_PROVIDER_DOWN`.
    dispatcher: Option<Arc<Dispatcher>>,
    /// Live Yjs sync provider state (DocRepo / DocStepRepo / PeerRepo over the main pool + the
    /// live-document registry). `None` only in the bare token-only skeleton.
    sync: Option<SyncState>,
}

impl AppState {
    /// Build state for the bare route skeleton with no module handles (auth-only unit tests).
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Inner {
                token: token.into().into_bytes(),
                services: None,
            }),
        }
    }

    /// Build the data/compute-only state from the boot token and the initialized [`BootServices`]
    /// (no AI dispatcher / sync provider). Synchronous; used by the wire1 data-flow tests which do
    /// not exercise `/llm` / `/document` / `/ws/sync`.
    pub fn from_boot(token: impl Into<String>, svc: BootServices) -> Self {
        Self::assemble(token, svc, None, None)
    }

    /// Build the fully-wired state: data/compute services + the M7 [`Dispatcher`] (from
    /// `secrets_path`'s `[siliconflow]`, wired to the live hsd / kb / rule-engine) + the live Yjs
    /// [`SyncState`] (its `doc_yjs_state` / `doc_step` / `peer` tables created on the main pool).
    ///
    /// Dispatcher construction is best-effort: a missing / unparseable `secrets.toml` leaves the
    /// dispatcher absent (so `/llm/query` returns `E_LLM_PROVIDER_DOWN`) without aborting boot — the
    /// data / compute / document / sync paths remain fully live.
    pub async fn from_boot_wired(
        token: impl Into<String>,
        svc: BootServices,
        secrets_path: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<Self> {
        // 1. Sync provider tables on the main store pool (idempotent DDL), then real repos.
        let pool = svc.store.pool().clone();
        ensure_schema(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("sync ensure_schema: {e}"))?;
        let sync_state = SyncState::new(
            Arc::new(SqliteDocRepo::new(pool.clone())),
            Arc::new(SqliteDocStepRepo::new(pool.clone())),
            Arc::new(SqlitePeerRepo::new(pool.clone())),
            Arc::new(SqliteDocMetaRepo::new(pool)),
        );

        // 2. AI dispatcher from secrets (best-effort; never logs / panics on the key).
        let dispatcher = match SecretsConfig::load(secrets_path.as_ref()) {
            Ok(secrets) => match Dispatcher::from_secrets(&secrets, None) {
                Ok(d) => Some(Arc::new(d)),
                Err(e) => {
                    tracing::warn!(error = %e, "dispatcher construction failed; /llm degraded");
                    None
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "secrets.toml unavailable; /llm degraded");
                None
            }
        };

        Ok(Self::assemble(token, svc, dispatcher, Some(sync_state)))
    }

    fn assemble(
        token: impl Into<String>,
        svc: BootServices,
        dispatcher: Option<Arc<Dispatcher>>,
        sync: Option<SyncState>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                token: token.into().into_bytes(),
                services: Some(Services {
                    store: svc.store,
                    audit: svc.audit,
                    audit_chain_ok: svc.audit_chain_ok,
                    rule_engine: svc.rule_engine,
                    diagnosis_engine: svc.diagnosis_engine,
                    hsd: svc.hsd,
                    kb_index: svc.kb_index,
                    kb_manifest: svc.kb_manifest,
                    kb_version: svc.kb_version,
                    age_store: svc.age_store,
                    session_dek: svc.session_dek,
                    dispatcher,
                    sync,
                }),
            }),
        }
    }

    /// Constant-time Bearer token comparison (backend/01 §1.1.3 step 3).
    ///
    /// `subtle::ConstantTimeEq` over equal-length byte slices avoids leaking the matched
    /// prefix length. A length mismatch returns `false` without a short-circuit on content.
    pub fn verify_token(&self, presented: &str) -> bool {
        let expected = &self.inner.token;
        let presented = presented.as_bytes();
        if expected.len() != presented.len() {
            return false;
        }
        expected.ct_eq(presented).into()
    }

    /// Borrow the main-store handle if module services are attached.
    pub fn store(&self) -> Option<&Store> {
        self.inner.services.as_ref().map(|s| &s.store)
    }

    /// Borrow the audit log if attached.
    pub fn audit(&self) -> Option<&AuditLog> {
        self.inner.services.as_ref().map(|s| &s.audit)
    }

    /// Whether the audit hash chain verified clean on startup. When `false`, write handlers refuse
    /// new audit appends and map to `E_AUDIT_CHAIN_BROKEN` (D3 / INV-06). Returns `false` when no
    /// services are attached.
    pub fn audit_chain_ok(&self) -> bool {
        self.inner
            .services
            .as_ref()
            .map(|s| s.audit_chain_ok)
            .unwrap_or(false)
    }

    /// Borrow the rule engine if attached.
    pub fn rule_engine(&self) -> Option<&RuleEngine> {
        self.inner.services.as_ref().map(|s| &s.rule_engine)
    }

    /// Borrow the deterministic M1 diagnosis engine if attached.
    pub fn diagnosis_engine(&self) -> Option<&DiagnosisEngine> {
        self.inner.services.as_ref().map(|s| &s.diagnosis_engine)
    }

    /// Borrow the high-sensitivity detector if attached.
    pub fn hsd(&self) -> Option<&HsdDetector> {
        self.inner.services.as_ref().map(|s| s.hsd.as_ref())
    }

    /// Borrow the KB search index if attached.
    pub fn kb_index(&self) -> Option<&Bm25Index> {
        self.inner.services.as_ref().map(|s| s.kb_index.as_ref())
    }

    /// Borrow the current KB manifest if attached.
    pub fn kb_manifest(&self) -> Option<&KbManifest> {
        self.inner.services.as_ref().map(|s| &s.kb_manifest)
    }

    /// Borrow the current KB version row if attached.
    pub fn kb_version(&self) -> Option<&KbVersion> {
        self.inner.services.as_ref().map(|s| &s.kb_version)
    }

    /// Borrow the age blob store if attached.
    pub fn age_store(&self) -> Option<&AgeStore> {
        self.inner.services.as_ref().map(|s| s.age_store.as_ref())
    }

    /// Borrow the M7 AI dispatcher if it was constructed (secrets present + valid).
    pub fn dispatcher(&self) -> Option<&Dispatcher> {
        self.inner
            .services
            .as_ref()
            .and_then(|s| s.dispatcher.as_deref())
    }

    /// Borrow the live Yjs sync provider state if attached.
    pub fn sync(&self) -> Option<&SyncState> {
        self.inner.services.as_ref().and_then(|s| s.sync.as_ref())
    }

    /// Whole-day age of the active KB (drives the dispatcher Stage-B Level4 gate). `0` when no KB.
    pub fn kb_age_days(&self) -> u32 {
        match self.kb_version() {
            Some(v) => kb::age_days(v.generated_at, chrono::Utc::now()),
            None => 0,
        }
    }

    /// A fresh trace id for an `ApiEnvelope`; v7 UUID string (time-sortable).
    pub fn new_trace_id(&self) -> String {
        Uuid::now_v7().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_token_matches_exact_only() {
        let st = AppState::new("deadbeef");
        assert!(st.verify_token("deadbeef"));
        assert!(!st.verify_token("deadbee"));
        assert!(!st.verify_token("deadbeef0"));
        assert!(!st.verify_token("DEADBEEF"));
        assert!(!st.verify_token(""));
    }

    #[test]
    fn trace_id_is_uuid() {
        let st = AppState::new("t");
        let id = st.new_trace_id();
        assert!(Uuid::parse_str(&id).is_ok(), "got {id}");
    }

    #[test]
    fn skeleton_state_has_no_services() {
        let st = AppState::new("t");
        assert!(st.store().is_none());
        assert!(st.rule_engine().is_none());
        assert!(!st.audit_chain_ok());
    }
}
