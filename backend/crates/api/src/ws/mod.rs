//! WebSocket upgrade routes (backend/01 §1.2, R1).
//!
//! `/ws/sync/:doc_id` (Yjs provider) and `/ws/status` (degradation-layer / KB-version push).
//! Both sit behind the Bearer middleware (`ipc::auth::require_bearer`) which runs **before** the
//! upgrade — so auth completes pre-accept (backend/01 §1.1.3). The post-accept handlers are
//! placeholders that accept the socket and close; the Yjs / status logic lands in `crates/sync`.

pub mod status;
pub mod sync;

use axum::Router;

use crate::state::AppState;

/// Merge the WS upgrade routes.
pub fn all() -> Router<AppState> {
    Router::new().merge(sync::routes()).merge(status::routes())
}
