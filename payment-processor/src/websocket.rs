use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde_json;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub type BroadcastSender = broadcast::Sender<String>;
pub type BroadcastReceiver = broadcast::Receiver<String>;

#[derive(Clone)]
pub struct WebSocketManager {
    tx: BroadcastSender,
}

impl WebSocketManager {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1000);
        Self { tx }
    }

    pub fn subscribe(&self) -> BroadcastReceiver {
        self.tx.subscribe()
    }

    pub async fn broadcast(&self, message: &str) {
        if let Err(e) = self.tx.send(message.to_string()) {
            warn!("Failed to broadcast message: {}", e);
        }
    }

    pub async fn broadcast_transaction_created(&self, transaction: &shared::Transaction) {
        let message = serde_json::json!({
            "type": "transaction_created",
            "transaction": transaction
        });
        self.broadcast(&message.to_string()).await;
    }

    pub async fn broadcast_transaction_updated(&self, transaction: &shared::Transaction) {
        let message = serde_json::json!({
            "type": "transaction_updated",
            "transaction": transaction
        });
        self.broadcast(&message.to_string()).await;
    }

    pub async fn broadcast_metrics(&self, metrics: &MetricsData) {
        let message = serde_json::json!({
            "type": "metrics_update",
            "metrics": metrics
        });
        self.broadcast(&message.to_string()).await;
    }
}

#[derive(Clone, serde::Serialize)]
pub struct MetricsData {
    pub total_transactions: i64,
    pub pending_transactions: i64,
    pub committed_transactions: i64,
    pub failed_transactions: i64,
    pub total_amount: i64,
    pub throughput: f64,
    pub avg_latency: f64,
    pub p95_latency: f64,
    pub error_rate: f64,
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(ws_manager): State<Arc<WebSocketManager>>,
) -> Response {
    info!("WebSocket upgrade request received");
    ws.on_upgrade(|socket| websocket_connection(socket, ws_manager))
}

async fn websocket_connection(socket: WebSocket, ws_manager: Arc<WebSocketManager>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = ws_manager.subscribe();

    info!("WebSocket client connected");

    // Send initial metrics
    let initial_metrics = MetricsData {
        total_transactions: 0,
        pending_transactions: 0,
        committed_transactions: 0,
        failed_transactions: 0,
        total_amount: 0,
        throughput: 0.0,
        avg_latency: 0.0,
        p95_latency: 0.0,
        error_rate: 0.0,
    };

    if let Ok(msg) = serde_json::to_string(&serde_json::json!({
        "type": "metrics_update",
        "metrics": initial_metrics
    })) {
        if let Err(e) = sender.send(Message::Text(msg)).await {
            warn!("Failed to send initial metrics: {}", e);
            return;
        }
    } else {
        warn!("Failed to serialize initial metrics");
        return;
    }

    // Handle incoming messages and broadcast messages
    tokio::select! {
        _ = async {
            while let Ok(msg) = rx.recv().await {
                if let Err(e) = sender.send(Message::Text(msg)).await {
                    warn!("Failed to send WebSocket message: {}", e);
                    break;
                }
            }
        } => {},
        _ = async {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(msg) => {
                        match msg {
                            Message::Text(text) => {
                                info!("Received WebSocket message: {}", text);
                            }
                            Message::Close(_) => {
                                info!("WebSocket client disconnected");
                                break;
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        warn!("WebSocket receive error: {}", e);
                        break;
                    }
                }
            }
        } => {}
    }

    info!("WebSocket connection closed");
}

