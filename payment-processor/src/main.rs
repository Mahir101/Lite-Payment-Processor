use anyhow::Result;
use axum::{
    extract::{Path, Query, State, ws::WebSocketUpgrade},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use shared::{
    ApiResponse, HealthCheck, HealthStatus, PaymentRequest, Transaction, TransactionEventType,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{info, warn, error};
use uuid::Uuid;

mod auth;
mod card_validation;
mod database;
mod metrics;
mod outbox;
mod redis_client;
mod state_machine;
mod user_management;
mod visa_api;
mod websocket;

use auth::AuthService;
use card_validation::{CardValidator, FraudDetector};
use database::DatabaseService;
use redis_client::RedisService;
use state_machine::TransactionStateMachine;
use user_management::UserService;
use visa_api::VisaApiClient;
use websocket::WebSocketManager;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseService,
    pub redis: RedisService,
    pub auth: AuthService,
    pub state_machine: TransactionStateMachine,
    pub ws_manager: std::sync::Arc<WebSocketManager>,
    pub user_service: UserService,
    pub fraud_detector: std::sync::Arc<FraudDetector>,
    pub visa_client: std::sync::Arc<VisaApiClient>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    info!("Starting Payment Processor Service");

    // Initialize metrics
    metrics::init_metrics();

    // Initialize services
    let db = DatabaseService::new().await?;
    let redis = RedisService::new().await?;
    let auth = AuthService::new();
    let state_machine = TransactionStateMachine::new();
    let ws_manager = std::sync::Arc::new(WebSocketManager::new());
    let user_service = UserService::new(db.pool.clone());
    let fraud_detector = std::sync::Arc::new(FraudDetector::new());
    let visa_client = std::sync::Arc::new(VisaApiClient::new("demo_api_key".to_string()));

    let app_state = AppState {
        db,
        redis,
        auth,
        state_machine,
        ws_manager,
        user_service,
        fraud_detector,
        visa_client,
    };

    // Start outbox processor
    let outbox_service = outbox::OutboxService::new(app_state.db.pool.clone());
    let outbox_processor = outbox::OutboxProcessor::new(outbox_service, app_state.redis.clone());
    tokio::spawn(async move {
        if let Err(e) = outbox_processor.start_processor().await {
            error!("Outbox processor failed: {}", e);
        }
    });

    // Build application
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/transactions", post(create_transaction))
        .route("/transactions/:id", get(get_transaction))
        .route("/transactions/:id/commit", post(commit_transaction))
        .route("/transactions/:id/fail", post(fail_transaction))
        .route("/transactions/:id/cancel", post(cancel_transaction))
        .route("/transactions", get(list_transactions))
        .route("/ws", get(websocket_handler))
        .route("/metrics", get(metrics_handler))
        // New card and user management endpoints
        .route("/users", post(create_user))
        .route("/users/:id", get(get_user))
        .route("/users/:id/verify", post(verify_user))
        .route("/users/:id/accounts", get(get_user_accounts))
        .route("/accounts", post(create_account))
        .route("/accounts/:number", get(get_account))
        .route("/transfer", post(transfer_money))
        .route("/validate-card", post(validate_card))
        .route("/visa-payment", post(process_visa_payment))
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default()))
                .layer(CorsLayer::permissive()),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    info!("Payment Processor Service listening on port 3001");
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check(State(state): State<AppState>) -> Json<ApiResponse<HealthCheck>> {
    let mut dependencies = HashMap::new();
    
    // Check database
    match state.db.health_check().await {
        Ok(_) => {
            dependencies.insert("database".to_string(), shared::DependencyHealth {
                status: HealthStatus::Healthy,
                response_time_ms: Some(10),
                last_check: Utc::now(),
            });
        }
        Err(e) => {
            dependencies.insert("database".to_string(), shared::DependencyHealth {
                status: HealthStatus::Unhealthy,
                response_time_ms: None,
                last_check: Utc::now(),
            });
            error!("Database health check failed: {}", e);
        }
    }

    // Check Redis
    match state.redis.health_check().await {
        Ok(_) => {
            dependencies.insert("redis".to_string(), shared::DependencyHealth {
                status: HealthStatus::Healthy,
                response_time_ms: Some(5),
                last_check: Utc::now(),
            });
        }
        Err(e) => {
            dependencies.insert("redis".to_string(), shared::DependencyHealth {
                status: HealthStatus::Unhealthy,
                response_time_ms: None,
                last_check: Utc::now(),
            });
            error!("Redis health check failed: {}", e);
        }
    }

    let overall_status = if dependencies.values().any(|d| d.status == HealthStatus::Unhealthy) {
        HealthStatus::Unhealthy
    } else if dependencies.values().any(|d| d.status == HealthStatus::Degraded) {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let health = HealthCheck {
        service: "payment-processor".to_string(),
        status: overall_status,
        timestamp: Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        dependencies,
    };

    Json(ApiResponse::success(health))
}

async fn create_transaction(
    State(state): State<AppState>,
    Json(payload): Json<PaymentRequest>,
) -> Result<Json<ApiResponse<Transaction>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Creating transaction for external_id: {}", payload.external_id);

    // Check idempotency
    let idempotency_key = format!("txn:{}", payload.external_id);
    if let Err(e) = state.redis.check_idempotency(&idempotency_key).await {
        warn!("Idempotency check failed: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error("Internal server error".to_string())),
        ));
    }

    // Create transaction
    match state.db.create_transaction(payload).await {
        Ok(transaction) => {
            // Set idempotency lock
            if let Err(e) = state.redis.set_idempotency_lock(&idempotency_key, &transaction.id).await {
                warn!("Failed to set idempotency lock: {}", e);
            }

            // Emit event
            if let Err(e) = state.db.emit_event(&transaction, TransactionEventType::Created).await {
                warn!("Failed to emit event: {}", e);
            }

            // Broadcast to WebSocket clients
            state.ws_manager.broadcast_transaction_created(&transaction).await;

            info!("Transaction created successfully: {}", transaction.id);
            Ok(Json(ApiResponse::success(transaction)))
        }
        Err(e) => {
            error!("Failed to create transaction: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ))
        }
    }
}

async fn get_transaction(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Transaction>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.db.get_transaction(id).await {
        Ok(Some(transaction)) => Ok(Json(ApiResponse::success(transaction))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Transaction not found".to_string())),
        )),
        Err(e) => {
            error!("Failed to get transaction: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn commit_transaction(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Transaction>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.state_machine.transition_to_committed(&state.db, id).await {
        Ok(transaction) => {
            info!("Transaction committed: {}", id);
            Ok(Json(ApiResponse::success(transaction)))
        }
        Err(e) => {
            error!("Failed to commit transaction: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ))
        }
    }
}

async fn fail_transaction(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Transaction>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.state_machine.transition_to_failed(&state.db, id, "Manual failure".to_string()).await {
        Ok(transaction) => {
            info!("Transaction failed: {}", id);
            Ok(Json(ApiResponse::success(transaction)))
        }
        Err(e) => {
            error!("Failed to fail transaction: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ))
        }
    }
}

async fn cancel_transaction(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Transaction>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.state_machine.transition_to_cancelled(&state.db, id).await {
        Ok(transaction) => {
            info!("Transaction cancelled: {}", id);
            Ok(Json(ApiResponse::success(transaction)))
        }
        Err(e) => {
            error!("Failed to cancel transaction: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ))
        }
    }
}

#[derive(Deserialize)]
struct ListTransactionsQuery {
    state: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_transactions(
    State(state): State<AppState>,
    Query(query): Query<ListTransactionsQuery>,
) -> Result<Json<ApiResponse<Vec<Transaction>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    match state.db.list_transactions(query.state, limit, offset).await {
        Ok(transactions) => Ok(Json(ApiResponse::success(transactions))),
        Err(e) => {
            error!("Failed to list transactions: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> axum::response::Response {
    websocket::websocket_handler(ws, State(state.ws_manager)).await
}

async fn metrics_handler() -> String {
    metrics::get_metrics()
}

// User Management Handlers

#[derive(serde::Deserialize)]
struct CreateUserRequest {
    email: String,
    phone: Option<String>,
    device_id: Option<String>,
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<ApiResponse<shared::UserInfo>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Creating user with email: {}", payload.email);

    match state.user_service.create_user(&payload.email, payload.phone.as_deref(), payload.device_id.as_deref()).await {
        Ok(user) => {
            info!("User created successfully: {}", user.id);
            Ok(Json(ApiResponse::success(user)))
        }
        Err(e) => {
            error!("Failed to create user: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ))
        }
    }
}

async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<shared::UserInfo>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.user_service.get_user_by_id(id).await {
        Ok(Some(user)) => Ok(Json(ApiResponse::success(user))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("User not found".to_string())),
        )),
        Err(e) => {
            error!("Failed to get user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn verify_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.user_service.verify_user(id).await {
        Ok(_) => {
            info!("User verified successfully: {}", id);
            Ok(Json(ApiResponse::success(())))
        }
        Err(e) => {
            error!("Failed to verify user: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ))
        }
    }
}

async fn get_user_accounts(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<shared::Account>>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.user_service.get_user_accounts(id).await {
        Ok(accounts) => Ok(Json(ApiResponse::success(accounts))),
        Err(e) => {
            error!("Failed to get user accounts: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

#[derive(serde::Deserialize)]
struct CreateAccountRequest {
    user_id: Uuid,
    account_type: String,
    currency: String,
    initial_balance: i64,
}

async fn create_account(
    State(state): State<AppState>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<Json<ApiResponse<shared::Account>>, (StatusCode, Json<ApiResponse<()>>)> {
    let account_type = match payload.account_type.as_str() {
        "CHECKING" => shared::AccountType::Checking,
        "SAVINGS" => shared::AccountType::Savings,
        "CREDIT" => shared::AccountType::Credit,
        "DEBIT" => shared::AccountType::Debit,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error("Invalid account type".to_string())),
            ));
        }
    };

    match state.user_service.create_account(payload.user_id, account_type, &payload.currency, payload.initial_balance).await {
        Ok(account) => {
            info!("Account created successfully: {}", account.account_number);
            Ok(Json(ApiResponse::success(account)))
        }
        Err(e) => {
            error!("Failed to create account: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ))
        }
    }
}

async fn get_account(
    State(state): State<AppState>,
    Path(account_number): Path<String>,
) -> Result<Json<ApiResponse<shared::Account>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.user_service.get_account_by_number(&account_number).await {
        Ok(Some(account)) => Ok(Json(ApiResponse::success(account))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Account not found".to_string())),
        )),
        Err(e) => {
            error!("Failed to get account: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn transfer_money(
    State(state): State<AppState>,
    Json(payload): Json<shared::TransferRequest>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Processing transfer from {} to {} for amount {}", payload.from_account, payload.to_account, payload.amount);

    // Validate card if provided
    if let Some(ref card_info) = payload.card_info {
        if let Err(e) = CardValidator::validate_card(card_info) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Card validation failed: {}", e))),
            ));
        }

        // Check for fraud
        if let Err(e) = state.fraud_detector.check_fraud(card_info, &payload.user_info) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(format!("Fraud detected: {}", e))),
            ));
        }
    }

    // Check if user exists and is verified
    if let Some(ref user_info) = payload.user_info {
        if !user_info.is_verified {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error("User not verified".to_string())),
            ));
        }
    }

    // Check sufficient funds
    match state.user_service.check_sufficient_funds(&payload.from_account, payload.amount).await {
        Ok(false) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error("Insufficient funds".to_string())),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ));
        }
        Ok(true) => {} // Continue with transfer
    }

    // Perform transfer
    match state.user_service.transfer_money(&payload.from_account, &payload.to_account, payload.amount).await {
        Ok(_) => {
            info!("Transfer completed successfully");
            Ok(Json(ApiResponse::success(())))
        }
        Err(e) => {
            error!("Failed to transfer money: {}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(e.to_string())),
            ))
        }
    }
}

async fn validate_card(
    State(_state): State<AppState>,
    Json(card_info): Json<shared::CardInfo>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Validating card ending in {}", &card_info.pan[card_info.pan.len()-4..]);

    match CardValidator::validate_card(&card_info) {
        Ok(_) => {
            let card_type = CardValidator::get_card_type(&card_info.pan);
            let masked_pan = CardValidator::mask_card_number(&card_info.pan);
            
            let response = serde_json::json!({
                "valid": true,
                "card_type": card_type,
                "masked_pan": masked_pan,
                "message": "Card is valid"
            });
            
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            let response = serde_json::json!({
                "valid": false,
                "error": e.to_string(),
                "message": "Card validation failed"
            });
            
            Ok(Json(ApiResponse::success(response)))
        }
    }
}

#[derive(serde::Deserialize)]
struct VisaPaymentRequest {
    card_info: shared::CardInfo,
    amount: i64,
    currency: String,
    description: Option<String>,
}

async fn process_visa_payment(
    State(state): State<AppState>,
    Json(payload): Json<VisaPaymentRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Processing Visa payment for amount {} {}", payload.amount, payload.currency);

    // Validate card first
    if let Err(e) = CardValidator::validate_card(&payload.card_info) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(format!("Card validation failed: {}", e))),
        ));
    }

    // Check for fraud
    if let Err(e) = state.fraud_detector.check_fraud(&payload.card_info, &None) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error(format!("Fraud detected: {}", e))),
        ));
    }

    // Process payment through Visa API
    match state.visa_client.process_payment(&payload.card_info, payload.amount, &payload.currency).await {
        Ok(visa_response) => {
            info!("Visa payment successful: {}", visa_response.transaction_id);
            
            let response = serde_json::json!({
                "success": true,
                "visa_transaction_id": visa_response.transaction_id,
                "auth_code": visa_response.auth_code,
                "status": visa_response.status,
                "response_code": visa_response.response_code,
                "response_message": visa_response.response_message,
                "amount": visa_response.amount,
                "currency": visa_response.currency,
                "timestamp": visa_response.timestamp,
                "card_type": CardValidator::get_card_type(&payload.card_info.pan),
                "masked_card": CardValidator::mask_card_number(&payload.card_info.pan)
            });
            
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Visa payment failed: {}", e);
            let response = serde_json::json!({
                "success": false,
                "error": e.to_string(),
                "card_type": CardValidator::get_card_type(&payload.card_info.pan),
                "masked_card": CardValidator::mask_card_number(&payload.card_info.pan)
            });
            
            Ok(Json(ApiResponse::success(response)))
        }
    }
}

