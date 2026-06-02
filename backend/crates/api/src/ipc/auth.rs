//! Bearer token middleware with intranet-penetration defenses (D1 · backend/01 §1.1.3).
//!
//! Fronts **all** routes including WebSocket upgrades, so auth completes before any handler or
//! WS accept runs. Three gates, in order:
//! 1. **Host** header must be loopback (`127.0.0.1` / `localhost`) → else 403 (anti DNS-rebinding);
//! 2. **Origin** header, when present, must be loopback → else 403 (anti cross-site);
//! 3. **Bearer** token constant-time compared to the handshake token → else 401 (E_INVALID_TOKEN).

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// Bearer middleware: loopback Host + loopback-or-absent Origin + constant-time Bearer.
pub async fn require_bearer(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = req.headers();

    // 1. Host loopback (anti DNS-rebinding). A missing Host on a forged request is rejected.
    if !is_loopback_host(headers) {
        return Err(StatusCode::FORBIDDEN);
    }
    // 2. Origin loopback or absent (anti cross-site).
    if !origin_is_loopback_or_absent(headers) {
        return Err(StatusCode::FORBIDDEN);
    }
    // 3. Bearer constant-time compare → E_INVALID_TOKEN (401).
    match bearer_token(headers) {
        Some(tok) if state.verify_token(tok) => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// True iff the `Host` header names a loopback address (`127.0.0.1` / `localhost`),
/// ignoring any `:port` suffix. A missing `Host` is treated as non-loopback (reject).
pub fn is_loopback_host(headers: &HeaderMap) -> bool {
    match headers.get("host").and_then(|v| v.to_str().ok()) {
        Some(host) => host_is_loopback(host),
        None => false,
    }
}

/// True iff the `Origin` header is absent, or present and loopback.
pub fn origin_is_loopback_or_absent(headers: &HeaderMap) -> bool {
    match headers.get("origin").and_then(|v| v.to_str().ok()) {
        None => true,
        Some(origin) => origin_is_loopback(origin),
    }
}

/// Extract the `Authorization: Bearer <token>` value, if present and well-formed.
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let raw = headers.get("authorization")?.to_str().ok()?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

/// `host[:port]` is loopback iff the host part is `127.0.0.1`, `localhost`, or `[::1]`/`::1`.
fn host_is_loopback(host: &str) -> bool {
    let host = host.trim();
    // IPv6 literal in brackets: `[::1]:port`.
    if let Some(rest) = host.strip_prefix('[') {
        if let Some((addr, _)) = rest.split_once(']') {
            return addr == "::1";
        }
        return false;
    }
    let name = host.split(':').next().unwrap_or(host);
    matches!(name, "127.0.0.1" | "localhost") || name == "::1"
}

/// An `Origin` like `http://127.0.0.1:54321` is loopback iff its host authority is loopback.
fn origin_is_loopback(origin: &str) -> bool {
    let origin = origin.trim();
    if origin.eq_ignore_ascii_case("null") {
        // Some webviews send "null"; treat as non-loopback (reject) to be safe.
        return false;
    }
    let authority = origin
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(origin);
    // Strip any path after the authority.
    let authority = authority.split('/').next().unwrap_or(authority);
    host_is_loopback(authority)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue};

    fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            let name = HeaderName::from_bytes(k.as_bytes()).unwrap();
            h.insert(name, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn loopback_host_variants() {
        assert!(is_loopback_host(&hm(&[("host", "127.0.0.1:54321")])));
        assert!(is_loopback_host(&hm(&[("host", "127.0.0.1")])));
        assert!(is_loopback_host(&hm(&[("host", "localhost:8080")])));
        assert!(is_loopback_host(&hm(&[("host", "[::1]:8080")])));
    }

    #[test]
    fn non_loopback_host_rejected() {
        assert!(!is_loopback_host(&hm(&[("host", "evil.example.com")])));
        assert!(!is_loopback_host(&hm(&[("host", "192.168.1.5:8080")])));
        assert!(
            !is_loopback_host(&HeaderMap::new()),
            "missing Host rejected"
        );
    }

    #[test]
    fn origin_absent_is_ok() {
        assert!(origin_is_loopback_or_absent(&HeaderMap::new()));
    }

    #[test]
    fn origin_loopback_ok_cross_site_rejected() {
        assert!(origin_is_loopback_or_absent(&hm(&[(
            "origin",
            "http://127.0.0.1:54321"
        )])));
        assert!(origin_is_loopback_or_absent(&hm(&[(
            "origin",
            "http://localhost:3000"
        )])));
        assert!(!origin_is_loopback_or_absent(&hm(&[(
            "origin",
            "http://evil.example.com"
        )])));
        assert!(!origin_is_loopback_or_absent(&hm(&[("origin", "null")])));
    }

    #[test]
    fn bearer_extraction() {
        assert_eq!(
            bearer_token(&hm(&[("authorization", "Bearer abc123")])),
            Some("abc123")
        );
        assert_eq!(
            bearer_token(&hm(&[("authorization", "bearer abc123")])),
            Some("abc123")
        );
        assert_eq!(
            bearer_token(&hm(&[("authorization", "Basic abc123")])),
            None
        );
        assert_eq!(bearer_token(&hm(&[("authorization", "Bearer ")])), None);
        assert_eq!(bearer_token(&HeaderMap::new()), None);
    }
}
