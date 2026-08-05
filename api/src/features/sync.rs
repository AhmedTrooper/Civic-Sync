//! Real-time sync layer for the §5C conditional flush driver.
//!
//! data.md §5C says the dashboard (Next.js / Flutter Admin) receives
//! updates once every 60 seconds, but only when underlying data has
//! changed. The flush driver in `flush.rs` already broadcasts a
//! `FlushNotice` over a `tokio::sync::broadcast::Sender` whenever a
//! 60-second window contained at least one mutation. This module exposes
//! that broadcast to clients over a WebSocket so the dashboard can
//! re-render without polling.
//!
//! ## Wire protocol
//!
//! Each connection:
//! 1. Receives one `{"type":"hello","now":"<rfc3339>"}` frame on connect.
//! 2. Receives one `{"type":"flush", ...FlushNotice}` frame every 60s
//!    that the broadcast channel publishes.
//! 3. Closes when the broadcast channel lags (more than 64 messages
//!    behind — `tokio::sync::broadcast`'s built-in safety net).

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};

use crate::state::AppState;

/// Upgrade any `GET /api/v1/sync/ws` request to a WebSocket. The handler
/// is intentionally cheap: it just hands the socket to [`run_socket`].
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| run_socket(socket, state))
}

async fn run_socket(socket: WebSocket, state: AppState) {
    let mut receiver = state.flush_notice_tx.subscribe();
    let (mut sender, mut socket) = socket.split();

    // 1) hello frame.
    let hello = serde_json::json!({
        "type": "hello",
        "now": chrono::Utc::now(),
        "flush_interval_seconds": crate::features::flush::FLUSH_INTERVAL.as_secs(),
    });
    if send_json(&mut sender, &hello).await.is_err() {
        return;
    }

    // 2) forward each FlushNotice as a `flush` frame.
    loop {
        match receiver.recv().await {
            Ok(notice) => {
                let frame = serde_json::json!({
                    "type": "flush",
                    "notice": notice,
                });
                if send_json(&mut sender, &frame).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(skipped, "ws sync subscriber lagged");
                let frame = serde_json::json!({
                    "type": "lagged",
                    "skipped": skipped,
                });
                if send_json(&mut sender, &frame).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                let _ = sender.send(Message::Close(None)).await;
                break;
            }
        }
        // Honour client-initiated close frames.
        if let Some(Ok(Message::Close(_))) = socket.next().await {
            break;
        }
    }
}

async fn send_json(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    payload: &serde_json::Value,
) -> Result<(), axum::Error> {
    let body = serde_json::to_vec(payload).unwrap_or_else(|_| b"{}".to_vec());
    sender.send(Message::Binary(body.into())).await
}
