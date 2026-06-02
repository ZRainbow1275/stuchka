//! R1 HTTP route registration (backend/01 §1.2 route table).
//!
//! Every R1 path/method is registered here with a contract-accurate DTO. Handlers whose module
//! logic is not yet wired return `501 E_NOT_IMPLEMENTED` (see [`crate::responses::not_implemented`]);
//! `/health` is fully live and returns a 200 [`data_model::ApiEnvelope`].
//!
//! WebSocket upgrades (`/ws/sync/:doc_id`, `/ws/status`) are registered separately in
//! [`crate::ws`].

pub mod audit;
pub mod case;
pub mod compute;
pub mod deadline;
pub mod diagnose;
pub mod document;
pub mod evidence;
pub mod fact;
pub mod health;
pub mod kb;
pub mod law_ref;
pub mod llm;
pub mod sync;

use axum::Router;

use crate::state::AppState;

/// Merge every R1 HTTP route module into one router.
pub fn all() -> Router<AppState> {
    Router::new()
        .merge(case::routes())
        .merge(diagnose::routes())
        .merge(fact::routes())
        .merge(evidence::routes())
        .merge(document::routes())
        .merge(law_ref::routes())
        .merge(audit::routes())
        .merge(sync::routes())
        .merge(llm::routes())
        .merge(compute::routes())
        .merge(deadline::routes())
        .merge(kb::routes())
        .merge(health::routes())
}
