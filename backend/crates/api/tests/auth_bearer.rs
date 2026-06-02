//! Bearer + loopback-guard middleware tests (backend/01 §1.1.3 verifiable acceptance).
//!
//! Mounts the full router in-process and drives it with `tower::ServiceExt::oneshot`, which lets
//! us set arbitrary `Host` / `Origin` / `Authorization` headers (a real client can't forge a
//! non-loopback Host against a loopback listener). Asserts, for both a normal route (`/health`)
//! and the `/ws/sync/:doc_id` upgrade path:
//! - missing Bearer → 401
//! - wrong Bearer → 401
//! - non-loopback Host → 403
//! - valid Bearer + loopback → passes the guard.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt; // for `oneshot`

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app() -> axum::Router {
    api::router(api::AppState::new(TOKEN))
}

/// Build a request to `path` with optional headers.
fn req(
    path: &str,
    host: Option<&str>,
    origin: Option<&str>,
    bearer: Option<&str>,
) -> Request<Body> {
    let mut b = Request::builder().uri(path).method("GET");
    if let Some(h) = host {
        b = b.header("host", h);
    }
    if let Some(o) = origin {
        b = b.header("origin", o);
    }
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

/// A request shaped like a WebSocket upgrade (without doing the real handshake) so we can prove
/// the Bearer guard runs *before* the upgrade. The guard returns 401/403 before any 101.
fn ws_upgrade_req(path: &str, host: Option<&str>, bearer: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .uri(path)
        .method("GET")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==");
    if let Some(h) = host {
        b = b.header("host", h);
    }
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn missing_bearer_is_401() {
    let resp = app()
        .oneshot(req("/health", Some("127.0.0.1:54321"), None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_bearer_is_401() {
    let resp = app()
        .oneshot(req(
            "/health",
            Some("127.0.0.1:54321"),
            None,
            Some("not-the-token"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn non_loopback_host_is_403() {
    let resp = app()
        .oneshot(req("/health", Some("evil.example.com"), None, Some(TOKEN)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn missing_host_is_403() {
    let resp = app()
        .oneshot(req("/health", None, None, Some(TOKEN)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_site_origin_is_403() {
    let resp = app()
        .oneshot(req(
            "/health",
            Some("127.0.0.1:54321"),
            Some("http://evil.example.com"),
            Some(TOKEN),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn valid_bearer_loopback_passes_to_health_200() {
    let resp = app()
        .oneshot(req(
            "/health",
            Some("127.0.0.1:54321"),
            Some("http://127.0.0.1:54321"),
            Some(TOKEN),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "valid token + loopback reaches /health"
    );
}

// ---- WS upgrade path (/ws/sync/:doc_id) is guarded the same way ----

#[tokio::test]
async fn ws_sync_missing_bearer_is_401() {
    let resp = app()
        .oneshot(ws_upgrade_req(
            "/ws/sync/doc-1",
            Some("127.0.0.1:54321"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "guard runs before WS upgrade"
    );
}

#[tokio::test]
async fn ws_sync_wrong_bearer_is_401() {
    let resp = app()
        .oneshot(ws_upgrade_req(
            "/ws/sync/doc-1",
            Some("127.0.0.1:54321"),
            Some("bad"),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn ws_sync_non_loopback_host_is_403() {
    let resp = app()
        .oneshot(ws_upgrade_req(
            "/ws/sync/doc-1",
            Some("evil.example.com"),
            Some(TOKEN),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn ws_sync_valid_bearer_loopback_passes_guard_into_upgrade() {
    // With valid auth + loopback Host the guard lets the request reach the WS upgrade extractor.
    // Driving the router via tower `oneshot` (no real TCP connection to hijack), axum's
    // `WebSocketUpgrade` cannot complete the protocol switch, so it returns 426 Upgrade Required
    // rather than 101. The load-bearing assertion is that the guard did NOT reject it (no
    // 401/403): the request reached the WS handler, proving auth runs before the upgrade.
    let resp = app()
        .oneshot(ws_upgrade_req(
            "/ws/sync/doc-1",
            Some("127.0.0.1:54321"),
            Some(TOKEN),
        ))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "guard must not 401 a valid request"
    );
    assert_ne!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "guard must not 403 a valid request"
    );
    assert_eq!(
        resp.status(),
        StatusCode::UPGRADE_REQUIRED,
        "request reached the WS upgrade extractor (426 in the oneshot harness, 101 over real TCP)"
    );
}

#[tokio::test]
async fn unknown_route_with_valid_auth_is_404_not_guard_rejection() {
    // Auth passes; routing then yields 404. Confirms the guard isn't masking routing.
    let resp = app()
        .oneshot(req(
            "/no-such-route",
            Some("127.0.0.1:54321"),
            None,
            Some(TOKEN),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
