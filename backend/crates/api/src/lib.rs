//! `api` — axum HTTP + WebSocket routes (127.0.0.1 + Bearer, D1).
//!
//! This crate is both a library and a binary:
//! - the library exposes [`AppState`] and [`router`] so integration tests can mount the full
//!   app in-process (and so `main.rs` stays thin);
//! - the binary (`src/main.rs`, `stuchka-core`) is the forked child: it binds `127.0.0.1:0`,
//!   prints the `READY{...}` line, and serves.
//!
//! The route skeleton is **contract-accurate**: every R1 path/method from backend/01 §1.2 is
//! registered with DTOs that match the spec, but handlers whose module logic is not yet wired
//! return `501 E_NOT_IMPLEMENTED` via [`responses::not_implemented`]. The Bearer middleware
//! ([`ipc::auth::require_bearer`]) fronts all routes including WS upgrades.

pub mod audit_helper;
pub mod ipc;
pub mod responses;
pub mod routes;
pub mod routing_audit_sink;
pub mod state;
pub mod sync_state;
pub mod ws;

pub use state::AppState;

use axum::Router;

/// Crate identity for boot diagnostics and CI dependency-graph assertions.
pub const CRATE_NAME: &str = "api";

/// Build the full application router: all R1 routes + WS upgrades, fronted by the Bearer
/// middleware (Host/Origin loopback guard + constant-time token compare).
///
/// The middleware is applied as a `from_fn_with_state` layer so it sees [`AppState`] and runs
/// before any route handler — including the `/ws/sync/:doc_id` and `/ws/status` upgrades.
pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(routes::all())
        .merge(ws::all())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            ipc::auth::require_bearer,
        ))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "api");
    }
}
