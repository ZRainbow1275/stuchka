//! Y.Doc repository (`CaseDoc`) + state-vector management + write-through merge threshold
//! (03-sync-yjs.md §3.3). Each case is one root `Y.Doc` whose guid namespace is `case:<uuid v7>`
//! (D9). The `facts` / `evidences` / `claims` keys are CRDT collection fields (not SQL table
//! names); `documents` maps each document id to its own ProseMirror `XmlFragment`.
//!
//! yrs version note (I-5): the spec wrote the old trait names `Map` / `Array` / `XmlFragment`; the
//! current yrs handle types are `MapRef` / `ArrayRef` / `XmlFragmentRef` (the bare `Map` / `Array`
//! identifiers are traits). We use the `Ref` handle types as the brief instructs. Snapshots and
//! incremental updates both use the **v2** encoding for space + to avoid v1/v2 decode mismatches.

use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{
    ArrayRef, Doc, GetString, MapRef, Options, ReadTxn, StateVector, Transact, Update,
    XmlFragmentRef,
};

use crate::error::{SyncError, SyncResult};
use crate::repo::DocRepo;

/// Merge threshold: compact once the pending-update count reaches this many (03-sync-yjs §3.3.2).
pub const MERGE_THRESHOLD_COUNT: usize = 50;
/// Merge threshold: compact once the pending-update byte total reaches this many.
pub const MERGE_THRESHOLD_BYTES: usize = 256 * 1024;

/// Build the Y.Doc guid namespace for a case (D9): `case:<uuid v7>`.
#[must_use]
pub fn case_doc_guid(case_id: &str) -> String {
    if case_id.starts_with("case:") {
        case_id.to_string()
    } else {
        format!("case:{case_id}")
    }
}

/// One case-scoped CRDT document and its root shared types (03-sync-yjs §3.3.1).
pub struct CaseDoc {
    /// Root document; `guid = case:<uuid v7>`.
    pub doc: Doc,
    /// `case_meta` map: status / region / etc.
    pub meta: MapRef,
    /// `facts` array of Y.Map entries (first-class objects).
    pub facts: ArrayRef,
    /// `evidences` array of Y.Map entries (metadata only; very-high-sensitivity blobs stay out,
    /// §3.10 / SY-13).
    pub evidences: ArrayRef,
    /// `claims` array of Y.Map entries.
    pub claims: ArrayRef,
    /// `documents` map: document id (UUID v7) -> ProseMirror `XmlFragment`.
    pub documents: MapRef,
}

impl CaseDoc {
    /// Create a fresh `CaseDoc` for `case_id`, initialising every root shared type so a remote peer
    /// observes a consistent structure on first sync.
    #[must_use]
    pub fn new(case_id: &str) -> Self {
        let opts = Options {
            guid: case_doc_guid(case_id).into(),
            // GC on (skip_gc = false) so compacted snapshots drop deleted nodes (§3.3.2).
            skip_gc: false,
            ..Options::default()
        };
        let doc = Doc::with_options(opts);
        Self::roots(doc)
    }

    /// Create a fresh `CaseDoc` with an explicit Yjs `client_id`.
    ///
    /// Yjs requires every collaborating peer to use a **distinct** client id, otherwise concurrent
    /// inserts collide and the store corrupts. Production should pass a stable per-device id
    /// (derived from the device key) so a device keeps the same client id across reconnects;
    /// [`CaseDoc::new`] uses a random id for one-shot / single-peer use.
    #[must_use]
    pub fn with_client_id(case_id: &str, client_id: u64) -> Self {
        let opts = Options {
            guid: case_doc_guid(case_id).into(),
            client_id: yrs::ClientID::new(client_id),
            skip_gc: false,
            ..Options::default()
        };
        let doc = Doc::with_options(opts);
        Self::roots(doc)
    }

    /// Restore a `CaseDoc` from a persisted snapshot (`doc_yjs_state.snapshot`, v2 encoded).
    pub fn open(case_id: &str, snapshot: &[u8]) -> SyncResult<Self> {
        let case_doc = Self::new(case_id);
        if !snapshot.is_empty() {
            case_doc.apply_update(snapshot)?;
        }
        Ok(case_doc)
    }

    fn roots(doc: Doc) -> Self {
        let meta = doc.get_or_insert_map("case_meta");
        let facts = doc.get_or_insert_array("facts");
        let evidences = doc.get_or_insert_array("evidences");
        let claims = doc.get_or_insert_array("claims");
        let documents = doc.get_or_insert_map("documents");
        Self {
            doc,
            meta,
            facts,
            evidences,
            claims,
            documents,
        }
    }

    /// The Y.Doc guid namespace string (`case:<uuid>`).
    #[must_use]
    pub fn guid(&self) -> String {
        self.doc.guid().to_string()
    }

    /// Apply a remote `Y.UpdateV2` to this document. Idempotent: re-applying an already-merged
    /// update is a no-op at the CRDT level (SY-02).
    pub fn apply_update(&self, update: &[u8]) -> SyncResult<()> {
        let update = Update::decode_v2(update)
            .or_else(|_| Update::decode_v1(update))
            .map_err(|e| SyncError::Yjs(format!("decode update: {e}")))?;
        let mut txn = self.doc.transact_mut();
        txn.apply_update(update)
            .map_err(|e| SyncError::Yjs(format!("apply update: {e}")))?;
        Ok(())
    }

    /// Encode the current state vector (`Y.encodeStateVector`), v1 framing.
    #[must_use]
    pub fn state_vector(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.state_vector().encode_v1()
    }

    /// Encode the delta this document holds beyond `remote_sv`
    /// (`Y.encodeStateAsUpdate(doc, remoteSV)`), v2 framing.
    pub fn diff_update(&self, remote_sv: &[u8]) -> SyncResult<Vec<u8>> {
        let sv = StateVector::decode_v1(remote_sv)
            .map_err(|e| SyncError::Yjs(format!("decode state vector: {e}")))?;
        let txn = self.doc.transact();
        Ok(txn.encode_state_as_update_v2(&sv))
    }

    /// Encode the full document as a single update from the empty state (snapshot rebuild, §3.3.2).
    #[must_use]
    pub fn encode_full(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.encode_state_as_update_v2(&StateVector::default())
    }

    /// Look up a document's ProseMirror fragment by id (creating it if absent — Y.Map get-or-init).
    #[must_use]
    pub fn doc_fragment(&self, doc_id: &str) -> XmlFragmentRef {
        // documents Map holds one XmlFragment per document id; create-or-get keeps callers simple.
        self.doc
            .get_or_insert_xml_fragment(format!("document:{doc_id}"))
    }

    /// The current plain-text body of a document's ProseMirror `XmlFragment` (the edited content
    /// persisted via `/ws/sync` + the write-through updates). Returns the real serialised fragment
    /// string — empty when the document has no body yet (a freshly-created, never-edited doc). Used
    /// by the export path to assemble the dossier over REAL content (no fabricated filler).
    #[must_use]
    pub fn document_text(&self, doc_id: &str) -> String {
        let fragment = self.doc_fragment(doc_id);
        let txn = self.doc.transact();
        fragment.get_string(&txn)
    }
}

/// Write-through a single update and compact when the merge threshold is hit (§3.3.2, SY-04).
///
/// Steps: append the update to `doc_yjs_state.pending_updates`; read `(count, bytes)`; if `count >=
/// 50` or `bytes >= 256KB`, rebuild the snapshot (`Y.encodeStateAsUpdate(doc)`), replace `snapshot`
/// / `state_vector`, clear pending, and set `snapshot_at = now()`.
pub async fn write_update(repo: &dyn DocRepo, doc_id: &str, update: &[u8]) -> SyncResult<()> {
    repo.append_update(doc_id, update).await?;
    let (count, bytes) = repo.pending_size(doc_id).await?;
    if count >= MERGE_THRESHOLD_COUNT || bytes >= MERGE_THRESHOLD_BYTES {
        compact(repo, doc_id).await?;
    }
    Ok(())
}

/// Rebuild the snapshot from the persisted snapshot + every pending update, then replace.
pub async fn compact(repo: &dyn DocRepo, doc_id: &str) -> SyncResult<()> {
    let snapshot = repo.load_snapshot(doc_id).await?.unwrap_or_default();
    let case_doc = CaseDoc::open(doc_id, &snapshot)?;
    for pending in repo.load_pending(doc_id).await? {
        case_doc.apply_update(&pending)?;
    }
    let new_snapshot = case_doc.encode_full();
    let new_sv = case_doc.state_vector();
    repo.compact(doc_id, &new_snapshot, &new_sv).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Array, Map, Transact};

    fn put_fact(doc: &CaseDoc, summary: &str) {
        let mut txn = doc.doc.transact_mut();
        let m = doc.facts.push_back(&mut txn, yrs::MapPrelim::default());
        m.insert(&mut txn, "summary", summary);
    }

    #[test]
    fn guid_uses_case_namespace() {
        let d = CaseDoc::new("0190b2aa-0000-7000-8000-000000000001");
        assert!(d.guid().starts_with("case:"), "got {}", d.guid());
        // Idempotent prefixing.
        let d2 = CaseDoc::new("case:already-prefixed");
        assert_eq!(d2.guid(), "case:already-prefixed");
    }

    #[test]
    fn state_vector_and_diff_round_trip() {
        // SY-03: A's diff_update(B.sv) applied to B converges B's SV to A's SV.
        let a = CaseDoc::new("c1");
        put_fact(&a, "A wrote this");

        let b = CaseDoc::new("c1");
        let b_sv = b.state_vector();
        let delta = a.diff_update(&b_sv).unwrap();
        b.apply_update(&delta).unwrap();

        assert_eq!(a.state_vector(), b.state_vector(), "SVs must converge");
        // And the data really transferred.
        let txn = b.doc.transact();
        assert_eq!(b.facts.len(&txn), 1);
    }

    #[test]
    fn apply_update_is_idempotent() {
        // SY-02: applying the same update twice leaves state unchanged (CRDT convergence).
        let a = CaseDoc::new("c1");
        put_fact(&a, "x");
        let full = a.encode_full();

        let b = CaseDoc::new("c1");
        b.apply_update(&full).unwrap();
        let sv_once = b.state_vector();
        b.apply_update(&full).unwrap();
        let sv_twice = b.state_vector();
        assert_eq!(sv_once, sv_twice, "re-applying an update must be a no-op");

        let txn = b.doc.transact();
        assert_eq!(b.facts.len(&txn), 1, "no duplicate fact from re-apply");
    }

    #[test]
    fn encode_full_round_trips_through_open() {
        let a = CaseDoc::new("c1");
        put_fact(&a, "persisted");
        let snap = a.encode_full();

        let reopened = CaseDoc::open("c1", &snap).unwrap();
        let txn = reopened.doc.transact();
        assert_eq!(reopened.facts.len(&txn), 1);
    }
}
