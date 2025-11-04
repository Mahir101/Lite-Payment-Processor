use actix_web::{web, HttpRequest, HttpResponse, Error};
use actix_web_actors::ws;
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

pub struct WebSocketActor {
    ws_manager: Arc<WebSocketManager>,
}

impl WebSocketActor {
    pub fn new(ws_manager: Arc<WebSocketManager>) -> Self {
        Self { ws_manager }
    }
}

impl actix::Actor for WebSocketActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("WebSocket client connected");
        
        // Subscribe to broadcast channel
        let mut rx = self.ws_manager.subscribe();
        let addr = ctx.address();
        
        // Spawn task to receive broadcast messages
        actix::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if let Err(_) = addr.send(WebSocketMessage(msg)).await {
                    break;
                }
            }
        });
    }
}

impl actix::StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                info!("Received WebSocket message: {}", text);
            }
            Ok(ws::Message::Close(reason)) => {
                info!("WebSocket client disconnected: {:?}", reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

#[derive(actix::Message)]
#[rtype(result = "()")]
struct WebSocketMessage(String);

impl actix::Handler<WebSocketMessage> for WebSocketActor {
    type Result = ();

    fn handle(&mut self, msg: WebSocketMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

pub async fn websocket_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<crate::AppState>,
) -> Result<HttpResponse, Error> {
    let actor = WebSocketActor::new(state.ws_manager.clone());
    let resp = ws::start(actor, &req, stream)?;
    Ok(resp)
}
