//! `sync` — Yjs CRDT (yrs) + WebSocket Provider (frame-level) + mDNS LAN discovery +
//! `.stuchka-patch` codec + ProseMirror Step audit double-write.
//!
//! Maps to master-index §0.4 (crate `sync`, related M3 / M8 / M15 / INV-06) and the backend specs
//! `03-sync-yjs.md` + `01-api-routes.md` §1.9. This crate is the **frame-level** sync engine: it
//! exposes pure logic (document repository, frame dispatch, discovery) so `crates/api` can inject
//! an axum `WebSocketUpgrade` after the Bearer + loopback gate (D1) and drive it without sync ever
//! depending on axum. Dependency direction (brief §1.2): sync -> { data-model, db, audit }; sync
//! never depends on api / rule-engine / ai-dispatcher / document / kb / hsd.
//!
//! Cross-crate rulings honoured: W2 time = chrono (the spec's `jiff::Timestamp` is overridden);
//! W5 random per-segment nonce lives in `crates/audit` (sync calls `AuditLog::append`); W7 sha2
//! 0.10 / sqlx 0.8 runtime API only; §E ed25519-dalek 2.1 is the one signature scheme (minisign is
//! reserved for upgrade packages — resolves spec self-contradiction I-6).
//!
//! ## R-phase boundary (brief §9)
//! - **R1a "made deep" (real here):** `CaseDoc` Y.Doc lifecycle + state-vector three-piece set +
//!   write-through persistence + 50/256KB merge threshold; the WS Provider frame dispatch (types
//!   0..=5) + `SyncHello` signature handshake + in-memory awareness; ProseMirror Step audit
//!   double-write (main `doc_step` + independent `audit.sqlite`, INV-06); mDNS advertise/browse
//!   with link-local filtering; `ConflictMarker` + `apply_merge_decision`.
//! - **R1a "made usable" (real, format frozen, deeper UX deferred to R1b):** `.stuchka-patch`
//!   encode/decode + ed25519 verify + authorization-chain `verify_chain` (file format + reserved
//!   data keys are frozen now); device pairing TOFU; multi-patch import; mobile provider.
//! - **R2 seam (declared only):** [`relay::RelayProvider`] is an `unimplemented!` trait (blocked by
//!   L0-02 备案).

pub mod auth;
pub mod mdns;
pub mod pairing;
pub mod patch;
pub mod provider;
pub mod relay;
pub mod repo;
pub mod yjs;

mod error;

pub use auth::{verify_chain, verify_patch_signature, AuthError, AuthLink};
pub use error::{SyncError, SyncResult};
pub use mdns::{advertise, browse, is_link_local, PeerDiscovered, SERVICE_TYPE};
pub use pairing::{handle_pair_request, PairRequest};
pub use patch::{
    decode as decode_patch, encode as encode_patch, PatchHeader, PatchScope, MAGIC, VERSION,
};
pub use provider::{
    apply_merge_decision, handle_frame, handle_step_audit, verify_hello, AnchorPath,
    ConflictMarker, FrameType, MergeDecision, OutboundFrame, PeerSession, StepAuditFrame,
    StepCategory, SyncHello, PROTOCOL_VERSION,
};
pub use repo::{
    ensure_schema, DocMetaRepo, DocRepo, DocStepRepo, DocStepRow, DocumentRow, PeerRepo, PeerRow,
    SqliteDocMetaRepo, SqliteDocRepo, SqliteDocStepRepo, SqlitePeerRepo,
};
pub use yjs::{write_update, CaseDoc, MERGE_THRESHOLD_BYTES, MERGE_THRESHOLD_COUNT};

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "sync";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "sync");
    }
}
