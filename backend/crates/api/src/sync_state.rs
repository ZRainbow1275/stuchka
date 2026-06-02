//! Live Yjs sync provider state (D1 · backend/01 §1.9 / 03-sync-yjs §3.3–3.5).
//!
//! `crates/sync` is the axum-free frame engine; this module holds the api-side runtime that wraps
//! it: the three persistence handles (`DocRepo` / `DocStepRepo` / `PeerRepo`, all real SQLite over
//! the **main** store pool, their tables created once at boot via `sync::ensure_schema`), an
//! in-memory registry of live [`sync::CaseDoc`]s (one per open document) so concurrent WS sessions
//! share one CRDT replica, and a per-document broadcast channel so an update applied by one peer is
//! fanned out to the others (03-sync-yjs §3.4.2 broadcast frames).
//!
//! The registry + channels are intentionally process-local (awareness / live sockets are never
//! persisted, §3.10 / SY-12). Document content itself is write-through-persisted by the repo.

use std::collections::HashMap;
use std::sync::Arc;

use sync::{CaseDoc, DocMetaRepo, DocRepo, DocStepRepo, PeerRepo};
use tokio::sync::{broadcast, Mutex};

/// Broadcast fan-out capacity per document channel (bounded; slow receivers lag and re-sync).
const BROADCAST_CAPACITY: usize = 256;

/// One open document: the shared CRDT replica + the broadcast channel feeding every live socket.
pub struct LiveDoc {
    /// The shared CRDT replica (guarded so frame application is serialised).
    pub case_doc: Arc<Mutex<CaseDoc>>,
    /// Broadcast of `(frame_type_byte, bytes)` to every other socket on this document.
    pub tx: broadcast::Sender<(u8, Vec<u8>)>,
}

/// The api-side sync runtime: persistence handles + the live-document registry.
#[derive(Clone)]
pub struct SyncState {
    inner: Arc<SyncInner>,
}

struct SyncInner {
    doc_repo: Arc<dyn DocRepo>,
    step_repo: Arc<dyn DocStepRepo>,
    peer_repo: Arc<dyn PeerRepo>,
    meta_repo: Arc<dyn DocMetaRepo>,
    /// doc_id -> live replica + broadcast channel.
    live: Mutex<HashMap<String, Arc<LiveDoc>>>,
}

impl SyncState {
    /// Build from the four real repositories (all over the main store pool).
    pub fn new(
        doc_repo: Arc<dyn DocRepo>,
        step_repo: Arc<dyn DocStepRepo>,
        peer_repo: Arc<dyn PeerRepo>,
        meta_repo: Arc<dyn DocMetaRepo>,
    ) -> Self {
        Self {
            inner: Arc::new(SyncInner {
                doc_repo,
                step_repo,
                peer_repo,
                meta_repo,
                live: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// The Yjs document-state repository.
    pub fn doc_repo(&self) -> &Arc<dyn DocRepo> {
        &self.inner.doc_repo
    }

    /// The ProseMirror Step mirror repository.
    pub fn step_repo(&self) -> &Arc<dyn DocStepRepo> {
        &self.inner.step_repo
    }

    /// The mDNS peer trust store.
    pub fn peer_repo(&self) -> &Arc<dyn PeerRepo> {
        &self.inner.peer_repo
    }

    /// The document business-metadata repository.
    pub fn meta_repo(&self) -> &Arc<dyn DocMetaRepo> {
        &self.inner.meta_repo
    }

    /// Get (or create) the live document for `doc_id`, hydrating it from the persisted snapshot +
    /// pending updates so a freshly-opened replica reflects everything written so far.
    pub async fn live_doc(&self, doc_id: &str) -> Result<Arc<LiveDoc>, sync::SyncError> {
        {
            let live = self.inner.live.lock().await;
            if let Some(d) = live.get(doc_id) {
                return Ok(d.clone());
            }
        }
        // Build outside the registry lock (CaseDoc creation + hydration can do I/O).
        let snapshot = self
            .inner
            .doc_repo
            .load_snapshot(doc_id)
            .await?
            .unwrap_or_default();
        let case_doc = CaseDoc::open(doc_id, &snapshot)?;
        for pending in self.inner.doc_repo.load_pending(doc_id).await? {
            case_doc.apply_update(&pending)?;
        }
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        let live_doc = Arc::new(LiveDoc {
            case_doc: Arc::new(Mutex::new(case_doc)),
            tx,
        });
        let mut live = self.inner.live.lock().await;
        // Re-check: another task may have inserted it while we hydrated.
        if let Some(existing) = live.get(doc_id) {
            return Ok(existing.clone());
        }
        live.insert(doc_id.to_string(), live_doc.clone());
        Ok(live_doc)
    }
}
