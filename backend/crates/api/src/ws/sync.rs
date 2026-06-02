//! `/ws/sync/:doc_id` — Yjs WebSocket provider upgrade (backend/01 §1.2, M3 / M15).
//!
//! Auth (Bearer + loopback Host/Origin) is enforced by `ipc::auth::require_bearer` before the
//! upgrade, so by the time this handler runs the request is the authenticated local editor. The
//! socket then drives the REAL frame-level engine in `crates/sync`: each inbound binary frame is
//! `[frame_type_byte | payload]`; we dispatch it through `sync::handle_frame` against the shared
//! live [`sync::CaseDoc`] for this document, write-through-persist applied updates
//! (`sync::write_update`), and fan broadcast frames out to every other socket on the document via
//! the per-doc broadcast channel (03-sync-yjs §3.4.2). Awareness stays in-memory (§3.10 / SY-12).
//!
//! The `SyncHello` signature handshake (peer trust) is for cross-device LAN sync (R1b pairing);
//! the single-device R1 editor is already authenticated by the Bearer gate, so the provider runs
//! without requiring a paired peer.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::Response,
    routing::get,
    Router,
};
use sync::{handle_frame, FrameType, PeerSession};

use crate::state::AppState;

/// Register `GET /ws/sync/:doc_id` (WS upgrade).
pub fn routes() -> Router<AppState> {
    Router::new().route("/ws/sync/:doc_id", get(sync_upgrade))
}

async fn sync_upgrade(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, doc_id))
}

/// Drive one Yjs provider socket against the shared live document.
async fn handle_socket(mut socket: WebSocket, state: AppState, doc_id: String) {
    let Some(sync) = state.sync() else {
        let _ = socket.close().await;
        return;
    };
    let live = match sync.live_doc(&doc_id).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, %doc_id, "ws sync: open live doc failed");
            let _ = socket.close().await;
            return;
        }
    };
    let mut rx = live.tx.subscribe();
    let mut session = PeerSession::new(doc_id.clone());

    loop {
        tokio::select! {
            // Inbound from this peer.
            inbound = socket.recv() => {
                let Some(Ok(msg)) = inbound else { break };
                match msg {
                    Message::Binary(buf) => {
                        if buf.is_empty() { continue; }
                        let Some(frame_type) = FrameType::from_u8(buf[0]) else { continue };
                        let payload = &buf[1..];
                        let case_doc = live.case_doc.lock().await;
                        let out = match handle_frame(&case_doc, &mut session, frame_type, payload) {
                            Ok(out) => out,
                            Err(e) => {
                                tracing::warn!(error = %e, %doc_id, "ws sync: frame dispatch failed");
                                continue;
                            }
                        };
                        drop(case_doc);
                        // Write-through persist any applied CRDT update (SyncStep2 / Update).
                        if matches!(frame_type, FrameType::SyncStep2 | FrameType::Update) {
                            if let Err(e) = sync::write_update(sync.doc_repo().as_ref(), &doc_id, payload).await {
                                tracing::warn!(error = %e, %doc_id, "ws sync: persist update failed");
                            }
                        }
                        for frame in out {
                            let mut bytes = Vec::with_capacity(frame.bytes.len() + 1);
                            bytes.push(frame.frame_type as u8);
                            bytes.extend_from_slice(&frame.bytes);
                            if frame.broadcast {
                                let _ = live.tx.send((frame.frame_type as u8, frame.bytes.clone()));
                            }
                            if socket.send(Message::Binary(bytes)).await.is_err() {
                                return;
                            }
                        }
                    }
                    Message::Close(_) => break,
                    Message::Ping(p) => {
                        let _ = socket.send(Message::Pong(p)).await;
                    }
                    _ => {}
                }
            }
            // Broadcast from other peers on this document.
            broadcast = rx.recv() => {
                match broadcast {
                    Ok((frame_type_byte, payload)) => {
                        let mut bytes = Vec::with_capacity(payload.len() + 1);
                        bytes.push(frame_type_byte);
                        bytes.extend_from_slice(&payload);
                        if socket.send(Message::Binary(bytes)).await.is_err() {
                            break;
                        }
                    }
                    // Lagged / closed: stop forwarding broadcasts (the client re-syncs on reconnect).
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    let _ = socket.close().await;
}
