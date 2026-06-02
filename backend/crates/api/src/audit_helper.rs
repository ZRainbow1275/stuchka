//! Audit append helpers (D3 / INV-06).
//!
//! The api layer is the writer of audit records for the deterministic engines and the db CRUD
//! (rule-engine never writes audit — INV-01). These helpers append a `who / when / why / what`
//! four-tuple to the independent `audit.sqlite` (`crates/audit`). When the startup chain
//! verification failed, writes are refused and the caller is handed an `E_AUDIT_CHAIN_BROKEN`
//! response (the audit log must not be extended on a broken chain).

use audit::{AuditReason, Subject};
use axum::response::Response;
use uuid::Uuid;

use crate::{responses, state::AppState};

/// The system subject for audit records produced by the backend on the user's behalf (R1a has no
/// per-user identity yet; the four-tuple `who` is the system component).
fn system_subject(component: &str) -> Subject {
    Subject::System {
        component: component.to_string(),
    }
}

/// Append a `state_change`-class audit record for a case-scoped reason. Returns `Err(Response)`
/// (an `E_AUDIT_CHAIN_BROKEN` 500 or `E_INTERNAL`) when the chain is broken or the append fails, so
/// the caller can short-circuit; `Ok(seq)` otherwise.
pub async fn append_state_change(
    state: &AppState,
    why: AuditReason,
    case_id: Uuid,
    what: serde_json::Value,
    trace: &str,
) -> Result<i64, Response> {
    append(state, system_subject("case"), why, case_id, what, trace).await
}

/// Append an audit record with an explicit subject. Shared by the state-change / evidence / fact
/// helpers.
pub async fn append(
    state: &AppState,
    who: Subject,
    why: AuditReason,
    _case_id: Uuid,
    what: serde_json::Value,
    trace: &str,
) -> Result<i64, Response> {
    if !state.audit_chain_ok() {
        return Err(responses::error(
            data_model::ErrorCode::AuditChainBroken,
            trace.to_string(),
        ));
    }
    let Some(audit) = state.audit() else {
        return Err(responses::error(
            data_model::ErrorCode::Internal,
            trace.to_string(),
        ));
    };
    audit.append(who, why, what).await.map_err(|e| {
        tracing::error!(error = %e, "audit append failed");
        responses::error(data_model::ErrorCode::Internal, trace.to_string())
    })
}
