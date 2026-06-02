//! WebSocket Provider server logic (03-sync-yjs §3.4 / §3.5 / §3.8), frame-level and axum-free.
//!
//! `crates/api` owns the axum `Router`, the Bearer + loopback gate (D1, api §1.1.3) and the
//! `WebSocketUpgrade`; after the gate it builds a [`PeerSession`] and feeds raw frame bytes to
//! [`handle_frame`], sending back each [`OutboundFrame`]. Keeping the engine on byte slices lets it
//! be unit-tested without a socket.
//!
//! INV-06 (§3.5): [`handle_step_audit`] double-writes — the business mirror to the main-store
//! `doc_step` table (via [`DocStepRepo`]) AND the authoritative four-tuple to the independent
//! `audit.sqlite` (via [`audit::AuditLog::append`]). I-3 ruling: the audit API is
//! `append(Subject, AuditReason, what)` (no `category` arg — it is derived from `why`); the spec's
//! `AuditCategory::MergeDecision` inline sketch is replaced by `AuditReason::DocumentMerged`
//! (merge decisions) / `AuditReason::AiDocumentDraft` (ai-accept) / `AuditReason::FactStateTransition`
//! (plain edit). Time is taken internally by audit (chrono, W2).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use audit::{AuditLog, AuditReason, Subject};
use ed25519_dalek::{Signature, VerifyingKey};

use crate::error::{SyncError, SyncResult};
use crate::repo::{DocStepRepo, DocStepRow, PeerRepo};
use crate::yjs::CaseDoc;

/// Sync protocol version (first handshake frame field).
pub const PROTOCOL_VERSION: u32 = 1;

/// First-frame handshake payload sent by the connecting client (§3.4.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncHello {
    /// Must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u32,
    /// ed25519 public key, base58 (the peer id; not subject to the D9 UUID rule, api §1.1.5).
    pub peer_id: String,
    /// ed25519 signature (base64) over `protocol_version|doc_id|nonce`.
    pub peer_signature: String,
    /// Client-chosen nonce.
    pub nonce: String,
    /// Client's current state vector (base64).
    pub state_vector_base64: String,
}

impl SyncHello {
    /// The canonical bytes the peer signature covers.
    #[must_use]
    pub fn signing_payload(&self, doc_id: &str) -> Vec<u8> {
        format!("{}|{}|{}", self.protocol_version, doc_id, self.nonce).into_bytes()
    }
}

/// y-websocket-compatible frame types (§3.4.2). 0..=3 are y-protocols standard; 4 is a full update
/// broadcast; 5 is the project's ProseMirror Step audit extension (§3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    /// SYNC step1 — state vector.
    SyncStep1 = 0,
    /// SYNC step2 — update answering a step1.
    SyncStep2 = 1,
    /// Awareness update (cursor / user state; memory only, §3.10 / SY-12).
    Awareness = 2,
    /// Query awareness.
    QueryAwareness = 3,
    /// Full update broadcast.
    Update = 4,
    /// ProseMirror Step audit (project extension, §3.5).
    StepAudit = 5,
}

impl FrameType {
    /// Decode the leading frame-type byte.
    #[must_use]
    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(FrameType::SyncStep1),
            1 => Some(FrameType::SyncStep2),
            2 => Some(FrameType::Awareness),
            3 => Some(FrameType::QueryAwareness),
            4 => Some(FrameType::Update),
            5 => Some(FrameType::StepAudit),
            _ => None,
        }
    }
}

/// A frame the engine wants the transport to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundFrame {
    /// Frame type byte the transport should prepend (or the caller can inspect).
    pub frame_type: FrameType,
    /// Frame body bytes.
    pub bytes: Vec<u8>,
    /// True if this frame should be broadcast to every other peer on the doc (e.g. updates,
    /// awareness); false if it is a direct reply to this peer (e.g. step2).
    pub broadcast: bool,
}

/// Per-connection session state, established after the Bearer gate + `SyncHello` handshake.
#[derive(Debug, Clone)]
pub struct PeerSession {
    /// The peer's base58 id.
    pub peer_id: String,
    /// The document this connection syncs.
    pub doc_id: String,
    /// Set true once `verify_hello` succeeds.
    pub authed: bool,
    /// Last-known remote state vector (v1 framed).
    pub remote_sv: Vec<u8>,
    /// In-memory awareness payload for this peer (never persisted, §3.10 / SY-12).
    pub awareness: Option<Vec<u8>>,
}

impl PeerSession {
    /// New, un-authed session for `doc_id`.
    #[must_use]
    pub fn new(doc_id: impl Into<String>) -> Self {
        Self {
            peer_id: String::new(),
            doc_id: doc_id.into(),
            authed: false,
            remote_sv: Vec::new(),
            awareness: None,
        }
    }
}

/// Verify a `SyncHello` (§3.4.1): version match, the peer must be trusted, and the peer signature
/// must verify under the peer's base58 ed25519 key.
pub async fn verify_hello(peers: &dyn PeerRepo, hello: &SyncHello, doc_id: &str) -> SyncResult<()> {
    if hello.protocol_version != PROTOCOL_VERSION {
        return Err(SyncError::Handshake(format!(
            "protocol version {} != {PROTOCOL_VERSION}",
            hello.protocol_version
        )));
    }
    if !peers.is_trusted(&hello.peer_id).await? {
        return Err(SyncError::Handshake("peer is not trusted".into()));
    }
    // The trusted pubkey is the peer id itself (base58 ed25519 key); a stored pubkey overrides.
    let key_b58 = peers
        .pubkey_of(&hello.peer_id)
        .await?
        .unwrap_or_else(|| hello.peer_id.clone());
    let key =
        decode_pubkey(&key_b58).ok_or_else(|| SyncError::Handshake("malformed peer key".into()))?;
    let sig = decode_signature(&hello.peer_signature)
        .ok_or_else(|| SyncError::Handshake("malformed signature".into()))?;
    key.verify_strict(&hello.signing_payload(doc_id), &sig)
        .map_err(|_| SyncError::Handshake("peer signature invalid".into()))?;
    Ok(())
}

fn decode_pubkey(peer_id: &str) -> Option<VerifyingKey> {
    let bytes = bs58::decode(peer_id).into_vec().ok()?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

fn decode_signature(b64: &str) -> Option<Signature> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    Signature::from_slice(&bytes).ok()
}

/// Dispatch one inbound frame against the live `case_doc` + this peer's `session` (§3.4.2).
///
/// - `SyncStep1`: the payload is the peer's state vector; reply `SyncStep2` with our delta.
/// - `SyncStep2` / `Update`: apply the update to the doc and broadcast it to other peers.
/// - `Awareness`: store in-memory and broadcast (never persisted, §3.10 / SY-12).
/// - `QueryAwareness`: reply with this peer's stored awareness (if any).
/// - `StepAudit`: not handled here (it carries a JSON body the api layer routes to
///   [`handle_step_audit`] with the audit + db handles); returns an empty vec.
pub fn handle_frame(
    case_doc: &CaseDoc,
    session: &mut PeerSession,
    frame_type: FrameType,
    payload: &[u8],
) -> SyncResult<Vec<OutboundFrame>> {
    match frame_type {
        FrameType::SyncStep1 => {
            session.remote_sv = payload.to_vec();
            let delta = case_doc.diff_update(payload)?;
            Ok(vec![OutboundFrame {
                frame_type: FrameType::SyncStep2,
                bytes: delta,
                broadcast: false,
            }])
        }
        FrameType::SyncStep2 | FrameType::Update => {
            case_doc.apply_update(payload)?;
            Ok(vec![OutboundFrame {
                frame_type: FrameType::Update,
                bytes: payload.to_vec(),
                broadcast: true,
            }])
        }
        FrameType::Awareness => {
            session.awareness = Some(payload.to_vec());
            Ok(vec![OutboundFrame {
                frame_type: FrameType::Awareness,
                bytes: payload.to_vec(),
                broadcast: true,
            }])
        }
        FrameType::QueryAwareness => Ok(session
            .awareness
            .clone()
            .map(|bytes| OutboundFrame {
                frame_type: FrameType::Awareness,
                bytes,
                broadcast: false,
            })
            .into_iter()
            .collect()),
        // The Step-audit frame body is JSON, not a CRDT update; the api WS handler decodes it and
        // calls handle_step_audit with the audit + db handles. Nothing to echo on the wire.
        FrameType::StepAudit => Ok(Vec::new()),
    }
}

/// Category of a ProseMirror Step audit (§3.5), mapping the front-end `why` triad onto the
/// authoritative [`AuditReason`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepCategory {
    /// A plain editor edit.
    Edit,
    /// The user accepted an AI suggestion.
    AiAccept,
    /// A three-way merge decision (INV-06 conflict resolution).
    MergeDecision,
}

impl StepCategory {
    /// The authoritative audit `why` for this category (I-3 mapping).
    #[must_use]
    pub fn audit_reason(self) -> AuditReason {
        match self {
            // Plain edits are case-scoped state changes.
            StepCategory::Edit => AuditReason::FactStateTransition,
            StepCategory::AiAccept => AuditReason::AiDocumentDraft,
            StepCategory::MergeDecision => AuditReason::DocumentMerged,
        }
    }
}

/// The ProseMirror Step audit frame (§3.5). `case_id` is required so the audit `what` carries the
/// D9 soft reference the audit schema validates against.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepAuditFrame {
    /// Document id (UUID v7).
    pub doc_id: Uuid,
    /// Owning case id (UUID v7) — required by the audit `what` schema.
    pub case_id: Uuid,
    /// Step number.
    pub step_no: i32,
    /// Full ProseMirror Step JSON.
    pub step_json: serde_json::Value,
    /// Yjs client id.
    pub client_id: String,
    /// Acting user id (UUID v7).
    pub actor: Uuid,
    /// Optional decision reason.
    pub reason: Option<String>,
    /// Edit / ai_accept / merge_decision.
    pub category: StepCategory,
}

/// INV-06 double-write (§3.5): mirror the Step to the main-store `doc_step` table, then append the
/// authoritative four-tuple to the independent `audit.sqlite` (D3).
///
/// The audit `who = Subject::User{actor}`, `why` is derived from `category` (I-3), and `what`
/// carries `{ case_id, doc_id, step_no, step }` so it passes the audit schema (which requires a
/// `case_id` for these reasons) and is queryable by case.
pub async fn handle_step_audit(
    db: &dyn DocStepRepo,
    audit: &AuditLog,
    frame: StepAuditFrame,
) -> SyncResult<i64> {
    // 1. Business mirror (main store, D9 singular `doc_step`).
    db.insert_doc_step(DocStepRow {
        doc_id: frame.doc_id,
        step_no: frame.step_no,
        step_json: frame.step_json.clone(),
        client_id: frame.client_id.clone(),
        actor: frame.actor.to_string(),
        reason: frame.reason.clone(),
    })
    .await?;

    // 2. Authoritative audit (independent audit.sqlite, hash-chained, D3 / INV-06).
    let what = serde_json::json!({
        "case_id": frame.case_id.to_string(),
        "doc_id": frame.doc_id.to_string(),
        "step_no": frame.step_no,
        "step": frame.step_json,
        "reason": frame.reason,
    });
    let seq = audit
        .append(
            Subject::User {
                user_id: frame.actor,
            },
            frame.category.audit_reason(),
            what,
        )
        .await
        .map_err(|e| SyncError::Audit(e.to_string()))?;
    Ok(seq)
}

/// ProseMirror anchor path for a conflict block (§3.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorPath {
    /// Node path from the document root.
    pub path: Vec<usize>,
    /// Inclusive start offset.
    pub from: usize,
    /// Exclusive end offset.
    pub to: usize,
}

/// A three-way conflict block surfaced to the front end (§3.8). The backend only *produces* these
/// markers and applies the chosen winner; the三栏 UI lives in the front end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictMarker {
    /// Conflict block id.
    pub conflict_id: String,
    /// Document id.
    pub doc_id: String,
    /// Where in the document the conflict sits.
    pub anchor: AnchorPath,
    /// Common-ancestor node JSON.
    pub ancestor: serde_json::Value,
    /// Local side.
    pub mine: serde_json::Value,
    /// Remote side.
    pub theirs: serde_json::Value,
}

/// A merge decision from `POST /document/:id/merge` (§3.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeDecision {
    /// Which conflict block this resolves.
    pub conflict_id: String,
    /// Winner: `mine` / `theirs` / `manual`.
    pub winner: String,
    /// Manual replacement text (when `winner == "manual"`).
    pub manual_text: Option<String>,
    /// The full ProseMirror Step JSON that applies the decision.
    pub step_json: serde_json::Value,
    /// Human reason for the decision.
    pub reason: Option<String>,
}

/// Apply a merge decision's Step to the document and return the update to broadcast (§3.8).
///
/// The decision's `step_json` is itself a Yjs/ProseMirror update payload; we apply it to the live
/// `case_doc` and hand back the post-decision delta against `before_sv` so the api layer can
/// broadcast it and then route the Step through [`handle_step_audit`] with
/// `StepCategory::MergeDecision`.
pub fn apply_merge_decision(
    case_doc: &CaseDoc,
    before_sv: &[u8],
    decision_update: &[u8],
) -> SyncResult<Vec<u8>> {
    case_doc.apply_update(decision_update)?;
    case_doc.diff_update(before_sv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use yrs::Transact;

    fn keypair(seed: u8) -> (SigningKey, String) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let peer_id = bs58::encode(sk.verifying_key().to_bytes()).into_string();
        (sk, peer_id)
    }

    fn signed_hello(sk: &SigningKey, peer_id: &str, doc_id: &str) -> SyncHello {
        let mut hello = SyncHello {
            protocol_version: PROTOCOL_VERSION,
            peer_id: peer_id.to_string(),
            peer_signature: String::new(),
            nonce: "nonce-1".into(),
            state_vector_base64: String::new(),
        };
        let sig = sk.sign(&hello.signing_payload(doc_id));
        hello.peer_signature = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
        hello
    }

    #[test]
    fn frame_type_round_trips() {
        for b in 0u8..=5 {
            assert_eq!(FrameType::from_u8(b).unwrap() as u8, b);
        }
        assert!(FrameType::from_u8(6).is_none());
    }

    #[test]
    fn step1_replies_step2_and_update_broadcasts() {
        let a = CaseDoc::new("c1");
        {
            let mut txn = a.doc.transact_mut();
            use yrs::{Array, Map};
            let m = a.facts.push_back(&mut txn, yrs::MapPrelim::default());
            m.insert(&mut txn, "summary", "hi");
        }
        let mut session = PeerSession::new("c1");

        // Empty SV step1 -> step2 carrying the full delta, direct (not broadcast).
        let empty_sv = {
            use yrs::updates::encoder::Encode;
            yrs::StateVector::default().encode_v1()
        };
        let out = handle_frame(&a, &mut session, FrameType::SyncStep1, &empty_sv).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].frame_type, FrameType::SyncStep2);
        assert!(!out[0].broadcast);

        // Applying that delta to a fresh doc converges, and an Update echo is a broadcast.
        let b = CaseDoc::new("c1");
        let mut bs = PeerSession::new("c1");
        let echo = handle_frame(&b, &mut bs, FrameType::Update, &out[0].bytes).unwrap();
        assert_eq!(echo[0].frame_type, FrameType::Update);
        assert!(echo[0].broadcast);
        assert_eq!(a.state_vector(), b.state_vector());
    }

    #[test]
    fn awareness_is_session_only() {
        // SY-12: awareness lives only in the session struct; nothing here touches a DB.
        let a = CaseDoc::new("c1");
        let mut session = PeerSession::new("c1");
        let out = handle_frame(&a, &mut session, FrameType::Awareness, b"cursor").unwrap();
        assert_eq!(session.awareness.as_deref(), Some(&b"cursor"[..]));
        assert!(out[0].broadcast);
        let q = handle_frame(&a, &mut session, FrameType::QueryAwareness, b"").unwrap();
        assert_eq!(q[0].bytes, b"cursor");
        assert!(!q[0].broadcast);
    }

    #[test]
    fn hello_signing_payload_is_stable() {
        let (sk, pid) = keypair(7);
        let hello = signed_hello(&sk, &pid, "doc-1");
        // The signature verifies under the peer's own key.
        let key = decode_pubkey(&pid).unwrap();
        let sig = decode_signature(&hello.peer_signature).unwrap();
        assert!(key
            .verify_strict(&hello.signing_payload("doc-1"), &sig)
            .is_ok());
        // ...and not under a different doc id.
        assert!(key
            .verify_strict(&hello.signing_payload("doc-2"), &sig)
            .is_err());
    }

    #[test]
    fn step_category_maps_to_audit_reason() {
        assert_eq!(
            StepCategory::MergeDecision.audit_reason(),
            AuditReason::DocumentMerged
        );
        assert_eq!(
            StepCategory::AiAccept.audit_reason(),
            AuditReason::AiDocumentDraft
        );
        assert_eq!(
            StepCategory::Edit.audit_reason(),
            AuditReason::FactStateTransition
        );
    }
}
