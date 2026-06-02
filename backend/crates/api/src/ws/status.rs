//! `/ws/status` — degradation-layer / KB-version push upgrade (backend/01 §1.2, R1).
//!
//! Auth is enforced by `ipc::auth::require_bearer` before the upgrade. The socket pushes the live
//! AI-dispatcher degrade level (ai/01 §1.8): an initial status frame on connect, then a fresh
//! [`ai_dispatcher::DegradeEvent`] whenever the dispatcher's current degrade level changes (every
//! degrade is visible / explainable / recoverable). The frame also carries the active KB version so
//! the front-end status bar can render `Level X · 原因 · 影响 N 模块 · 如何恢复` + the KB freshness.

use std::time::Duration;

use ai_dispatcher::{DegradeEvent, DegradeLevel, ReasonCode, RecoveryHint};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use serde::Serialize;

use crate::state::AppState;

/// Poll interval for the dispatcher degrade level (the FSM ticks every 60s; a 2s poll surfaces a
/// transition promptly without busy-waiting).
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// The status frame pushed over `/ws/status`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusFrame {
    /// Current degrade level (0-4).
    level: u8,
    /// The degrade-transition event (status-bar line included).
    event: DegradeEvent,
    /// Status-bar line (`Level X · 原因 · 影响 N 模块 · 如何恢复`).
    status_bar_line: String,
    /// Active KB version label.
    kb_version: Option<String>,
}

/// Register `GET /ws/status` (WS upgrade).
pub fn routes() -> Router<AppState> {
    Router::new().route("/ws/status", get(status_upgrade))
}

async fn status_upgrade(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Map a degrade level to the (reason, recovery) pair the status bar renders for it.
fn reason_recovery(level: DegradeLevel) -> (ReasonCode, RecoveryHint) {
    match level {
        DegradeLevel::Level0 => (ReasonCode::Recovered, RecoveryHint::RetryProvider),
        DegradeLevel::Level1 => (ReasonCode::PrimaryTimeout, RecoveryHint::RetryProvider),
        DegradeLevel::Level2 => (ReasonCode::Secondary429, RecoveryHint::RenewQuota),
        DegradeLevel::Level3 => (ReasonCode::LocalMissing, RecoveryHint::DownloadLocal),
        DegradeLevel::Level4 => (ReasonCode::KbStale, RecoveryHint::UpdateKb),
    }
}

fn build_frame(from: DegradeLevel, to: DegradeLevel, kb_version: Option<String>) -> StatusFrame {
    let (reason, recovery) = reason_recovery(to);
    let event = DegradeEvent::new(from, to, reason, recovery);
    StatusFrame {
        level: to.as_u8(),
        status_bar_line: event.status_bar_line(),
        event,
        kb_version,
    }
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let kb_version = state.kb_version().map(|v| v.version_label.clone());

    // Initial level (Level0 when no dispatcher is configured — the data/compute path is unaffected).
    let mut current = state
        .dispatcher()
        .map(|d| d.current_level())
        .unwrap_or(DegradeLevel::Level0);

    // Push the initial status frame.
    let frame = build_frame(current, current, kb_version.clone());
    if send_json(&mut socket, &frame).await.is_err() {
        return;
    }

    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    loop {
        tokio::select! {
            inbound = socket.recv() => {
                match inbound {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => { let _ = socket.send(Message::Pong(p)).await; }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            _ = ticker.tick() => {
                let next = state
                    .dispatcher()
                    .map(|d| d.current_level())
                    .unwrap_or(DegradeLevel::Level0);
                if next != current {
                    let frame = build_frame(current, next, kb_version.clone());
                    current = next;
                    if send_json(&mut socket, &frame).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
    let _ = socket.close().await;
}

async fn send_json<T: Serialize>(socket: &mut WebSocket, value: &T) -> Result<(), ()> {
    let text = serde_json::to_string(value).map_err(|_| ())?;
    socket.send(Message::Text(text)).await.map_err(|_| ())
}
