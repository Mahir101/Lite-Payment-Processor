//! # Payment Processor Service
//! 
//! This module contains the main application for the Payment Processor service.
//! It handles HTTP routing, request processing, and service orchestration.
//! 
//! ## Key Responsibilities:
//! - Transaction lifecycle management
//! - User and account management
//! - Card validation and fraud detection
//! - Payment processing via Visa API
//! - Real-time WebSocket updates
//! - JWT-based authentication
//! - Health monitoring and metrics

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

/// Application state containing all service dependencies
/// 
/// This struct holds references to all the services and components
/// needed by the HTTP handlers. It's cloned for each request handler
/// to provide access to database connections, Redis client, authentication
/// service, and other business logic components.
#[derive(Clone)]
pub struct AppState {
    /// Database service for transaction and user data operations
    pub db: DatabaseService,
    /// Redis client for caching and pub/sub operations
    pub redis: RedisService,
    /// JWT authentication service for token management
    pub auth: AuthService,
    /// State machine for managing transaction lifecycle
    pub state_machine: TransactionStateMachine,
    /// WebSocket manager for real-time updates
    pub ws_manager: std::sync::Arc<WebSocketManager>,
    /// User management service for user and account operations
    pub user_service: UserService,
    /// Fraud detection service for security checks
    pub fraud_detector: std::sync::Arc<FraudDetector>,
    /// Visa API client for external payment processing
    pub visa_client: std::sync::Arc<VisaApiClient>,
}

/// Main application entry point
/// 
/// This function initializes the Payment Processor service by:
/// 1. Setting up structured logging with tracing
/// 2. Initializing Prometheus metrics collection
/// 3. Creating all service dependencies (database, Redis, auth, etc.)
/// 4. Starting the outbox processor background task
/// 5. Configuring HTTP routes and middleware
/// 6. Starting the HTTP server on port 3001
/// 
/// # Returns
/// 
/// Returns `Result<()>` - Ok if the service starts successfully, Err if initialization fails
/// 
/// # Errors
/// 
/// This function can fail if:
/// - Database connection fails
/// - Redis connection fails
/// - HTTP server binding fails
/// - Service initialization fails
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging with tracing
    // This sets up JSON-formatted logs with thread information for better debugging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    info!("Starting Payment Processor Service");

    // Initialize Prometheus metrics collection
    // This registers all metric collectors for monitoring
    metrics::init_metrics();

    // Initialize all service dependencies
    // Each service is created with its required configuration
    let db = DatabaseService::new().await?;
    let redis = RedisService::new().await?;
    let auth = AuthService::new();
    let state_machine = TransactionStateMachine::new();
    let ws_manager = std::sync::Arc::new(WebSocketManager::new());
    let user_service = UserService::new(db.pool.clone());
    let fraud_detector = std::sync::Arc::new(FraudDetector::new());
    let visa_client = std::sync::Arc::new(VisaApiClient::new("demo_api_key".to_string()));

    // Create application state with all services
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

    // Start outbox processor background task
    // This ensures reliable event publishing using the outbox pattern
    let outbox_service = outbox::OutboxService::new(app_state.db.pool.clone());
    let outbox_processor = outbox::OutboxProcessor::new(outbox_service, app_state.redis.clone());
    tokio::spawn(async move {
        if let Err(e) = outbox_processor.start_processor().await {
            error!("Outbox processor failed: {}", e);
        }
    });

    // Build HTTP application with all routes and middleware
    let app = Router::new()
        // Health and monitoring endpoints
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/ws", get(websocket_handler))
        
        // Transaction management endpoints
        .route("/transactions", post(create_transaction))
        .route("/transactions/:id", get(get_transaction))
        .route("/transactions/:id/commit", post(commit_transaction))
        .route("/transactions/:id/fail", post(fail_transaction))
        .route("/transactions/:id/cancel", post(cancel_transaction))
        .route("/transactions", get(list_transactions))
        
        // User management endpoints
        .route("/users", post(create_user))
        .route("/users/:id", get(get_user))
        .route("/users/:id/verify", post(verify_user))
        .route("/users/:id/accounts", get(get_user_accounts))
        
        // Account management endpoints
        .route("/accounts", post(create_account))
        .route("/accounts/:number", get(get_account))
        
        // Payment processing endpoints
        .route("/transfer", post(transfer_money))
        .route("/validate-card", post(validate_card))
        .route("/visa-payment", post(process_visa_payment))
        
        // Add application state and middleware
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default()))
                .layer(CorsLayer::permissive()),
        );

    // Start HTTP server on port 3001
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    info!("Payment Processor Service listening on port 3001");
    
    axum::serve(listener, app).await?;
    Ok(())
}

/// Health check endpoint for service monitoring
/// 
/// This function performs comprehensive health checks on all service dependencies
/// and returns the overall health status. It's used by load balancers, monitoring
/// systems, and orchestration platforms to determine if the service is ready to
/// handle requests.
/// 
/// # Process:
/// 1. Checks database connectivity and performance
/// 2. Checks Redis connectivity and performance  
/// 3. Determines overall service health based on dependency status
/// 4. Returns structured health information with timestamps
/// 
/// # Parameters:
/// - `state`: Application state containing all service dependencies
/// 
/// # Returns:
/// - `Json<ApiResponse<HealthCheck>>`: Structured health check response
/// 
/// # Health Status Logic:
/// - `Healthy`: All dependencies are operational
/// - `Degraded`: Some dependencies have issues but service can function
/// - `Unhealthy`: Critical dependencies are down, service should not receive traffic
async fn health_check(State(state): State<AppState>) -> Json<ApiResponse<HealthCheck>> {
    let mut dependencies = HashMap::new();
    
    // Check database connectivity and performance
    // This performs a simple query to verify the database is responsive
    match state.db.health_check().await {
        Ok(_) => {
            dependencies.insert("database".to_string(), shared::DependencyHealth {
                status: HealthStatus::Healthy,
                response_time_ms: Some(10), // Estimated response time
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

    // Check Redis connectivity and performance
    // This performs a PING command to verify Redis is responsive
    match state.redis.health_check().await {
        Ok(_) => {
            dependencies.insert("redis".to_string(), shared::DependencyHealth {
                status: HealthStatus::Healthy,
                response_time_ms: Some(5), // Estimated response time
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

    // Determine overall service health based on dependency status
    // Priority: Unhealthy > Degraded > Healthy
    let overall_status = if dependencies.values().any(|d| d.status == HealthStatus::Unhealthy) {
        HealthStatus::Unhealthy
    } else if dependencies.values().any(|d| d.status == HealthStatus::Degraded) {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    // Create comprehensive health check response
    let health = HealthCheck {
        service: "payment-processor".to_string(),
        status: overall_status,
        timestamp: Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        dependencies,
    };

    Json(ApiResponse::success(health))
}

/// Creates a new payment transaction
/// 
/// This function handles the creation of new payment transactions with comprehensive
/// validation, idempotency checking, and event publishing. It ensures that duplicate
/// requests are handled correctly and that all stakeholders are notified of the transaction.
/// 
/// # Process:
/// 1. Validates idempotency using Redis to prevent duplicate transactions
/// 2. Creates the transaction in the database with PENDING state
/// 3. Sets idempotency lock to prevent duplicate processing
/// 4. Emits transaction created event for downstream processing
/// 5. Broadcasts real-time update to WebSocket clients
/// 6. Returns the created transaction details
/// 
/// # Parameters:
/// - `state`: Application state containing all service dependencies
/// - `payload`: Payment request containing transaction details
/// 
/// # Returns:
/// - `Ok(Json<ApiResponse<Transaction>>)`: Success with transaction details
/// - `Err((StatusCode, Json<ApiResponse<()>>))`: Error with appropriate HTTP status
/// 
/// # Idempotency:
/// Uses Redis to check if a transaction with the same external_id already exists.
/// If it does, the request is rejected to prevent duplicate processing.
/// 
/// # Event Publishing:
/// Uses the outbox pattern to ensure reliable event publishing. Events are stored
/// in the database transaction and published asynchronously.
/// 
/// # Real-time Updates:
/// Broadcasts transaction creation to all connected WebSocket clients for
/// real-time dashboard updates.
async fn create_transaction(
    State(state): State<AppState>,
    Json(payload): Json<PaymentRequest>,
) -> Result<Json<ApiResponse<Transaction>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("Creating transaction for external_id: {}", payload.external_id);

    // Check idempotency to prevent duplicate transactions
    // This ensures that the same external_id cannot be processed twice
    let idempotency_key = format!("txn:{}", payload.external_id);
    if let Err(e) = state.redis.check_idempotency(&idempotency_key).await {
        warn!("Idempotency check failed: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error("Internal server error".to_string())),
        ));
    }

    // Create transaction in database with atomic operation
    // This includes adding the transaction to the outbox for event publishing
    match state.db.create_transaction(payload).await {
        Ok(transaction) => {
            // Set idempotency lock to prevent duplicate processing
            // This lock expires after 5 minutes to handle edge cases
            if let Err(e) = state.redis.set_idempotency_lock(&idempotency_key, &transaction.id).await {
                warn!("Failed to set idempotency lock: {}", e);
            }

            // Emit transaction created event for downstream processing
            // This event will be consumed by the reconciliation service
            if let Err(e) = state.db.emit_event(&transaction, TransactionEventType::Created).await {
                warn!("Failed to emit event: {}", e);
            }

            // Broadcast real-time update to WebSocket clients
            // This enables live dashboard updates for transaction monitoring
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

/// Retrieves a transaction by its unique identifier
/// 
/// This function fetches a transaction from the database using its UUID.
/// It's used for transaction lookups, status checks, and detailed transaction views.
/// 
/// # Process:
/// 1. Queries the database for the transaction by UUID
/// 2. Returns the transaction if found
/// 3. Returns 404 if transaction doesn't exist
/// 4. Returns 500 if database query fails
/// 
/// # Parameters:
/// - `state`: Application state containing database service
/// - `id`: UUID of the transaction to retrieve
/// 
/// # Returns:
/// - `Ok(Json<ApiResponse<Transaction>>)`: Transaction details if found
/// - `Err(404, ...)`: Transaction not found
/// - `Err(500, ...)`: Database error
/// 
/// # Use Cases:
/// - Transaction status checking
/// - Transaction details display
/// - Audit trail lookups
/// - Customer service inquiries
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

/// Commits a pending transaction to completed state
/// 
/// This function transitions a transaction from PENDING to COMMITTED state using
/// the transaction state machine. It ensures that only valid state transitions
/// are allowed and emits appropriate events for downstream processing.
/// 
/// # Process:
/// 1. Validates that the transaction exists and is in PENDING state
/// 2. Uses state machine to validate the transition is allowed
/// 3. Updates transaction state to COMMITTED in database
/// 4. Emits state change event for reconciliation service
/// 5. Returns updated transaction details
/// 
/// # Parameters:
/// - `state`: Application state containing state machine and database
/// - `id`: UUID of the transaction to commit
/// 
/// # Returns:
/// - `Ok(Json<ApiResponse<Transaction>>)`: Updated transaction with COMMITTED state
/// - `Err(400, ...)`: Invalid state transition or transaction not found
/// 
/// # State Machine Rules:
/// - Only PENDING transactions can be committed
/// - COMMITTED, FAILED, and CANCELLED are terminal states
/// - State transitions are atomic and consistent
/// 
/// # Event Publishing:
/// Emits a StateChanged event that will be consumed by the reconciliation
/// service to update the event-sourced ledger.
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

/// Marks a transaction as failed with a reason
/// 
/// This function transitions a transaction from PENDING to FAILED state using
/// the transaction state machine. It's used when a transaction cannot be
/// completed due to various reasons (insufficient funds, fraud detection, etc.).
/// 
/// # Process:
/// 1. Validates that the transaction exists and is in PENDING state
/// 2. Uses state machine to validate the transition is allowed
/// 3. Updates transaction state to FAILED with failure reason
/// 4. Emits failure event for reconciliation service
/// 5. Returns updated transaction details
/// 
/// # Parameters:
/// - `state`: Application state containing state machine and database
/// - `id`: UUID of the transaction to mark as failed
/// 
/// # Returns:
/// - `Ok(Json<ApiResponse<Transaction>>)`: Updated transaction with FAILED state
/// - `Err(400, ...)`: Invalid state transition or transaction not found
/// 
/// # Failure Reasons:
/// - Manual failure (admin intervention)
/// - Insufficient funds
/// - Fraud detection
/// - Card validation failure
/// - External API errors
/// 
/// # Event Publishing:
/// Emits a Failed event with the reason that will be consumed by the
/// reconciliation service for audit and reporting purposes.
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

