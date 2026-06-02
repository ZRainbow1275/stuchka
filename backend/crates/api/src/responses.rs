//! Response helpers mapping [`data_model::ApiEnvelope`] / [`data_model::ErrorCode`] onto axum
//! responses (backend/01 §1.1.2 + §1.12).
//!
//! Success → HTTP 200/201 + `{ data, error: null, traceId }`; failure → HTTP 4xx/5xx +
//! `{ data: null, error: {...}, traceId }`. The error HTTP status is taken from the §1.12 table
//! ([`ErrorCode::http_status`]).

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use data_model::{ApiEnvelope, ApiError, ErrorCode};
use serde::Serialize;

/// Build a success response with an explicit HTTP status (200 or 201).
pub fn ok<T: Serialize>(status: StatusCode, data: T, trace_id: impl Into<String>) -> Response {
    let envelope = ApiEnvelope::ok(data, trace_id);
    (status, Json(envelope)).into_response()
}

/// Build a 200 success response.
pub fn ok_200<T: Serialize>(data: T, trace_id: impl Into<String>) -> Response {
    ok(StatusCode::OK, data, trace_id)
}

/// Build a failure response from a known [`ErrorCode`], using its §1.12 HTTP status and the
/// default Chinese message template.
pub fn error(code: ErrorCode, trace_id: impl Into<String>) -> Response {
    error_with(ApiError::from_code(code), code.http_status(), trace_id)
}

/// Build a failure response from a pre-built [`ApiError`] and explicit HTTP status.
pub fn error_with(err: ApiError, http_status: u16, trace_id: impl Into<String>) -> Response {
    let status = StatusCode::from_u16(http_status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: ApiEnvelope<()> = ApiEnvelope::err(err, trace_id);
    (status, Json(envelope)).into_response()
}

/// The contract-accurate `501 E_NOT_IMPLEMENTED` placeholder for routes whose module logic is
/// not yet wired. Path/method/DTO already match the spec; only the body logic is pending.
pub fn not_implemented(trace_id: impl Into<String>) -> Response {
    error(ErrorCode::NotImplemented, trace_id)
}

/// `201 Created` success response.
pub fn created<T: Serialize>(data: T, trace_id: impl Into<String>) -> Response {
    ok(StatusCode::CREATED, data, trace_id)
}

/// Map a [`db::DbError`] onto the appropriate §1.12 error response.
pub fn from_db_error(e: &db::DbError, trace_id: impl Into<String>) -> Response {
    use db::DbError;
    let code = match e {
        DbError::NotFound => ErrorCode::NotFound,
        DbError::InvalidTransition(_) | DbError::KbVersionFrozen => ErrorCode::InvalidTransition,
        DbError::Sqlx(_) | DbError::Json(_) | DbError::Decimal(_) | DbError::Uuid(_) => {
            ErrorCode::Internal
        }
    };
    error_with(
        ApiError::from_code(code).with_detail(serde_json::json!({ "detail": e.to_string() })),
        code.http_status(),
        trace_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_implemented_status_is_501() {
        assert_eq!(ErrorCode::NotImplemented.http_status(), 501);
    }
}
