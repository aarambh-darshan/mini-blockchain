//! WebSocket support for real-time blockchain updates
//!
//! Provides a broadcast channel for pushing events to connected clients.

use crate::api::handlers::{BlockInfo, TransactionResponse};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Maximum number of events to buffer per subscriber
const BROADCAST_CAPACITY: usize = 100;

/// WebSocket events that can be broadcast to clients
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    /// A new block was mined
    BlockMined { block: BlockInfo, reward: u64 },
    /// A new transaction was added to the mempool
    TransactionAdded { transaction: TransactionResponse },
    /// Chain state was updated
    ChainUpdated {
        height: u64,
        latest_hash: String,
        total_transactions: usize,
    },
    /// Connection established
    Connected { message: String },
    /// Heartbeat to keep connection alive
    Ping,
}

/// Broadcaster for WebSocket events
#[derive(Debug)]
pub struct WsBroadcaster {
    sender: broadcast::Sender<WsEvent>,
}

impl WsBroadcaster {
    /// Create a new broadcaster
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self { sender }
    }

    /// Broadcast an event to all connected clients
    pub fn broadcast(&self, event: WsEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.sender.send(event);
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.sender.subscribe()
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for WsBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<crate::api::handlers::ApiState>,
) -> impl IntoResponse {
    let broadcaster = state.ws_broadcaster.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, broadcaster))
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, broadcaster: Arc<WsBroadcaster>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast events
    let mut rx = broadcaster.subscribe();

    // Send welcome message
    let welcome = WsEvent::Connected {
        message: "Connected to Mini-Blockchain WebSocket".to_string(),
    };
    if let Ok(json) = serde_json::to_string(&welcome) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Spawn task to forward broadcast events to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages (for ping/pong and graceful close)
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(data)) => {
                    // Pong is handled automatically by axum
                    log::debug!("Received ping: {:?}", data);
                }
                Ok(Message::Text(text)) => {
                    log::debug!("Received text message: {}", text);
                }
                Err(e) => {
                    log::warn!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
        }
        _ = &mut recv_task => {
            send_task.abort();
        }
    }

    log::info!("WebSocket connection closed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcaster_creation() {
        let broadcaster = WsBroadcaster::new();
        assert_eq!(broadcaster.subscriber_count(), 0);
    }

    #[test]
    fn test_broadcast_with_no_subscribers() {
        let broadcaster = WsBroadcaster::new();
        // Should not panic even with no subscribers
        broadcaster.broadcast(WsEvent::Ping);
    }

    #[test]
    fn test_event_serialization() {
        let event = WsEvent::BlockMined {
            block: BlockInfo {
                index: 1,
                hash: "abc123".to_string(),
                previous_hash: "000000".to_string(),
                merkle_root: "merkle".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                difficulty: 16,
                nonce: 12345,
                transactions: 1,
            },
            reward: 50,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("BlockMined"));
        assert!(json.contains("abc123"));
    }
}
