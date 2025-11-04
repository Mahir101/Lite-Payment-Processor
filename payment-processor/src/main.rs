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
use actix_web::{
    web, App, HttpServer, HttpResponse, HttpRequest, Result as ActixResult,
    middleware::Logger, guard,
};
use actix_cors::Cors;
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use shared::{
    ApiResponse, HealthCheck, HealthStatus, PaymentRequest, Transaction, TransactionEventType,
};
use tracing::{info, warn, error};
use uuid::Uuid;
use std::sync::Arc;

mod auth;
mod card_validation;
mod database;
mod metrics;
mod metrics_middleware;
mod outbox;
mod redis_client;
mod state_machine;
mod user_management;
mod visa_api;
mod websocket;
mod refunds;
mod customers;
mod payment_methods;
mod webhooks;
mod subscriptions;
mod disputes;
mod invoicing;
mod payouts;
mod connect;
mod payment_intents;
mod advanced_fraud;
mod currency;
mod tax;
mod rate_limiting;
mod test_mode;

use auth::AuthService;
use card_validation::{CardValidator, FraudDetector};
use database::DatabaseService;
use redis_client::RedisService;
use state_machine::TransactionStateMachine;
use user_management::UserService;
use visa_api::VisaApiClient;
use websocket::WebSocketManager;
use refunds::RefundService;
use customers::CustomerService;
use payment_methods::PaymentMethodService;
use webhooks::WebhookService;
use subscriptions::SubscriptionService;
use disputes::DisputeService;
use invoicing::InvoiceService;
use payouts::PayoutService;
use connect::ConnectService;
use payment_intents::PaymentIntentService;
use advanced_fraud::AdvancedFraudDetector;
use currency::CurrencyService;
use tax::TaxService;
use rate_limiting::RateLimiter;
use test_mode::TestModeService;

/// Application state containing all service dependencies
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseService,
    pub redis: RedisService,
    pub auth: AuthService,
    pub state_machine: TransactionStateMachine,
    pub ws_manager: Arc<WebSocketManager>,
    pub user_service: UserService,
    pub fraud_detector: Arc<FraudDetector>,
    pub visa_client: Arc<VisaApiClient>,
    pub refund_service: RefundService,
    pub customer_service: CustomerService,
    pub payment_method_service: PaymentMethodService,
    pub webhook_service: WebhookService,
    pub subscription_service: SubscriptionService,
    pub dispute_service: DisputeService,
    pub invoice_service: InvoiceService,
    pub payout_service: PayoutService,
    pub connect_service: ConnectService,
    pub payment_intent_service: PaymentIntentService,
    pub advanced_fraud_detector: Arc<AdvancedFraudDetector>,
    pub currency_service: CurrencyService,
    pub tax_service: TaxService,
    pub test_mode_service: TestModeService,
    pub rate_limiter: Arc<RateLimiter>,
}

#[actix_web::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    info!("Starting Payment Processor Service");

    metrics::init_metrics();

    let db = DatabaseService::new().await?;
    let redis = RedisService::new().await?;
    let auth = AuthService::new();
    let state_machine = TransactionStateMachine::new();
    let ws_manager = Arc::new(WebSocketManager::new());
    let user_service = UserService::new(db.pool.clone());
    let fraud_detector = Arc::new(FraudDetector::new());
    let visa_client = Arc::new(VisaApiClient::new("demo_api_key".to_string()));
    let refund_service = RefundService::new(db.pool.clone());
    let customer_service = CustomerService::new(db.pool.clone());
    let payment_method_service = PaymentMethodService::new(db.pool.clone());
    let webhook_service = WebhookService::new(db.pool.clone());
    let subscription_service = SubscriptionService::new(db.pool.clone());
    let dispute_service = DisputeService::new(db.pool.clone());
    let invoice_service = InvoiceService::new(db.pool.clone());
    let payout_service = PayoutService::new(db.pool.clone());
    let connect_service = ConnectService::new(db.pool.clone());
    let payment_intent_service = PaymentIntentService::new(db.pool.clone());
    let advanced_fraud_detector = Arc::new(AdvancedFraudDetector::new());
    let currency_service = CurrencyService::new(db.pool.clone());
    let tax_service = TaxService::new(db.pool.clone());
    let test_mode_service = TestModeService::new(db.pool.clone());
    let rate_limiter = Arc::new(RateLimiter::new(db.pool.clone(), 100, 60));

    let app_state = web::Data::new(AppState {
        db,
        redis,
        auth,
        state_machine,
        ws_manager,
        user_service,
        fraud_detector,
        visa_client,
        refund_service,
        customer_service,
        payment_method_service,
        webhook_service,
        subscription_service,
        dispute_service,
        invoice_service,
        payout_service,
        connect_service,
        payment_intent_service,
        advanced_fraud_detector,
        currency_service,
        tax_service,
        test_mode_service,
        rate_limiter,
    });

    // Start outbox processor
    let outbox_service = outbox::OutboxService::new(app_state.db.pool.clone());
    let outbox_processor = outbox::OutboxProcessor::new(outbox_service.clone(), app_state.redis.clone());
    
    if let Err(e) = outbox_service.reset_failed_events(3).await {
        warn!("Failed to reset failed events: {}", e);
    }
    
    tokio::spawn(async move {
        if let Err(e) = outbox_processor.start_processor().await {
            error!("Outbox processor failed: {}", e);
        }
    });

    info!("Payment Processor Service listening on port 3001");
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(Logger::default())
            .wrap(metrics_middleware::MetricsMiddleware)
            .wrap(rate_limiting::RateLimitMiddleware::new(rate_limiter.clone()))
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600)
            )
            .service(
                web::scope("")
                    // Health and monitoring
                    .route("/health", web::get().to(health_check))
                    .route("/metrics", web::get().to(metrics_handler))
                    .route("/ws", web::get().to(websocket_handler))
                    
                    // Transactions
                    .route("/transactions", web::post().to(create_transaction))
                    .route("/transactions/{id}", web::get().to(get_transaction))
                    .route("/transactions/{id}/commit", web::post().to(commit_transaction))
                    .route("/transactions/{id}/fail", web::post().to(fail_transaction))
                    .route("/transactions/{id}/cancel", web::post().to(cancel_transaction))
                    .route("/transactions", web::get().to(list_transactions))
                    
                    // Users
                    .route("/users", web::post().to(create_user))
                    .route("/users/{id}", web::get().to(get_user))
                    .route("/users/{id}/verify", web::post().to(verify_user))
                    .route("/users/{id}/accounts", web::get().to(get_user_accounts))
                    
                    // Accounts
                    .route("/accounts", web::post().to(create_account))
                    .route("/accounts/{number}", web::get().to(get_account))
                    
                    // Payments
                    .route("/transfer", web::post().to(transfer_money))
                    .route("/validate-card", web::post().to(validate_card))
                    .route("/visa-payment", web::post().to(process_visa_payment))
                    
                    // Refunds
                    .route("/transactions/{id}/refund", web::post().to(create_refund))
                    .route("/refunds/{id}", web::get().to(get_refund))
                    .route("/transactions/{id}/refunds", web::get().to(list_refunds))
                    
                    // Customers
                    .route("/customers", web::post().to(create_customer))
                    .route("/customers/{id}", web::get().to(get_customer))
                    .route("/customers/{id}", web::put().to(update_customer))
                    .route("/customers", web::get().to(list_customers))
                    .route("/customers/{id}", web::delete().to(delete_customer))
                    
                    // Payment Methods
                    .route("/payment-methods", web::post().to(create_payment_method))
                    .route("/payment-methods/{id}", web::get().to(get_payment_method))
                    .route("/payment-methods", web::get().to(list_payment_methods))
                    .route("/payment-methods/{id}", web::delete().to(delete_payment_method))
                    .route("/customers/{customer_id}/payment-methods/{id}/default", web::post().to(set_default_payment_method))
                    
                    // Webhooks
                    .route("/webhooks", web::post().to(create_webhook))
                    .route("/webhooks/{id}", web::get().to(get_webhook))
                    .route("/webhooks", web::get().to(list_webhooks))
                    .route("/webhook", web::post().to(handle_webhook_delivery))
                    
                    // Subscriptions
                    .route("/subscriptions", web::post().to(create_subscription))
                    .route("/subscriptions/{id}", web::get().to(get_subscription))
                    .route("/subscriptions", web::get().to(list_subscriptions))
                    .route("/subscriptions/{id}/cancel", web::post().to(cancel_subscription))
                    
                    // Disputes
                    .route("/disputes", web::post().to(create_dispute))
                    .route("/disputes/{id}", web::get().to(get_dispute))
                    .route("/disputes", web::get().to(list_disputes))
                    .route("/disputes/{id}/submit-evidence", web::post().to(submit_dispute_evidence))
                    .route("/disputes/{id}/update-status", web::post().to(update_dispute_status))
                    
                    // Invoices
                    .route("/invoices", web::post().to(create_invoice))
                    .route("/invoices/{id}", web::get().to(get_invoice))
                    .route("/invoices", web::get().to(list_invoices))
                    .route("/invoices/{id}/finalize", web::post().to(finalize_invoice))
                    .route("/invoices/{id}/pay", web::post().to(mark_invoice_paid))
                    .route("/invoices/{id}/line-items", web::get().to(get_invoice_line_items))
                    
                    // Payouts
                    .route("/payouts", web::post().to(create_payout))
                    .route("/payouts/{id}", web::get().to(get_payout))
                    .route("/payouts", web::get().to(list_payouts))
                    .route("/payouts/{id}/cancel", web::post().to(cancel_payout))
                    
                    // Connect/Marketplace
                    .route("/connect/accounts", web::post().to(create_connect_account))
                    .route("/connect/accounts/{id}", web::get().to(get_connect_account))
                    .route("/connect/accounts/{id}/update", web::post().to(update_connect_account))
                    .route("/transfers", web::post().to(create_transfer))
                    .route("/transactions/{id}/transfers", web::get().to(list_transfers))
                    
                    // Payment Intents
                    .route("/payment-intents", web::post().to(create_payment_intent))
                    .route("/payment-intents/{id}", web::get().to(get_payment_intent))
                    .route("/payment-intents/{id}/confirm", web::post().to(confirm_payment_intent))
                    .route("/payment-intents/{id}/cancel", web::post().to(cancel_payment_intent))
                    .route("/payment-intents/{id}/3d-secure", web::post().to(handle_3d_secure))
                    
                    // Currency
                    .route("/currency/convert", web::post().to(convert_currency))
                    .route("/currency/exchange-rates", web::get().to(get_exchange_rate))
                    .route("/currency/exchange-rates", web::post().to(set_exchange_rate))
                    .route("/currency/supported", web::get().to(get_supported_currencies))
                    
                    // Tax
                    .route("/tax/calculate", web::post().to(calculate_tax))
                    .route("/tax/rates", web::post().to(create_tax_rate))
                    .route("/tax/rates", web::get().to(list_tax_rates))
                    
                    // Test Mode
                    .route("/test-mode/status", web::get().to(get_test_mode_status))
                    .route("/test-mode/enable", web::post().to(enable_test_mode))
                    .route("/test-mode/cards", web::get().to(get_test_cards))
            )
    })
    .bind("0.0.0.0:3001")?
    .run()
    .await?;

    Ok(())
}

// ========== HEALTH & METRICS HANDLERS ==========

async fn health_check(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let mut dependencies = HashMap::new();
    
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

    Ok(HttpResponse::Ok().json(ApiResponse::success(health)))
}

async fn metrics_handler() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().content_type("text/plain").body(metrics::get_metrics()))
}

async fn websocket_handler(req: HttpRequest, stream: web::Payload, state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    websocket::websocket_handler(req, stream, state).await
}

// ========== TRANSACTION HANDLERS ==========

async fn create_transaction(
    state: web::Data<AppState>,
    payload: web::Json<PaymentRequest>,
) -> ActixResult<HttpResponse> {
    info!("Creating transaction for external_id: {}", payload.external_id);

    let idempotency_key = format!("txn:{}", payload.external_id);
    if let Err(e) = state.redis.check_idempotency(&idempotency_key).await {
        warn!("Idempotency check failed: {}", e);
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::error("Internal server error".to_string())));
    }

    match state.db.create_transaction(payload.into_inner()).await {
        Ok(transaction) => {
            if let Err(e) = state.redis.set_idempotency_lock(&idempotency_key, &transaction.id).await {
                warn!("Failed to set idempotency lock: {}", e);
            }

            if let Err(e) = state.db.emit_event(&transaction, TransactionEventType::Created).await {
                warn!("Failed to emit event: {}", e);
            }

            metrics::increment_transaction_created(&transaction.currency);
            metrics::add_transaction_amount(transaction.amount);
            metrics::increment_outbox_event();

            state.ws_manager.broadcast_transaction_created(&transaction).await;

            info!("Transaction created successfully: {}", transaction.id);
            Ok(HttpResponse::Ok().json(ApiResponse::success(transaction)))
        }
        Err(e) => {
            metrics::increment_error("transaction_creation", "payment_processor");
            error!("Failed to create transaction: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
        }
    }
}

async fn get_transaction(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.db.get_transaction(path.into_inner()).await {
        Ok(Some(transaction)) => Ok(HttpResponse::Ok().json(ApiResponse::success(transaction))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Transaction not found".to_string()))),
        Err(e) => {
            error!("Failed to get transaction: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

async fn commit_transaction(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.state_machine.transition_to_committed(&state.db, path.into_inner()).await {
        Ok(transaction) => {
            metrics::increment_transaction_committed();
            info!("Transaction committed: {}", path.into_inner());
            Ok(HttpResponse::Ok().json(ApiResponse::success(transaction)))
        }
        Err(e) => {
            metrics::increment_error("transaction_commit", "payment_processor");
            error!("Failed to commit transaction: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
        }
    }
}

async fn fail_transaction(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.state_machine.transition_to_failed(&state.db, path.into_inner(), "Manual failure".to_string()).await {
        Ok(transaction) => {
            metrics::increment_transaction_failed();
            info!("Transaction failed: {}", path.into_inner());
            Ok(HttpResponse::Ok().json(ApiResponse::success(transaction)))
        }
        Err(e) => {
            metrics::increment_error("transaction_fail", "payment_processor");
            error!("Failed to fail transaction: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
        }
    }
}

async fn cancel_transaction(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.state_machine.transition_to_cancelled(&state.db, path.into_inner()).await {
        Ok(transaction) => {
            metrics::increment_transaction_cancelled();
            info!("Transaction cancelled: {}", path.into_inner());
            Ok(HttpResponse::Ok().json(ApiResponse::success(transaction)))
        }
        Err(e) => {
            metrics::increment_error("transaction_cancel", "payment_processor");
            error!("Failed to cancel transaction: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
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
    state: web::Data<AppState>,
    query: web::Query<ListTransactionsQuery>,
) -> ActixResult<HttpResponse> {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    match state.db.list_transactions(query.state.clone(), limit, offset).await {
        Ok(transactions) => Ok(HttpResponse::Ok().json(ApiResponse::success(transactions))),
        Err(e) => {
            error!("Failed to list transactions: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

// ========== USER HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateUserRequest {
    email: String,
    phone: Option<String>,
    device_id: Option<String>,
}

async fn create_user(
    state: web::Data<AppState>,
    payload: web::Json<CreateUserRequest>,
) -> ActixResult<HttpResponse> {
    info!("Creating user with email: {}", payload.email);

    match state.user_service.create_user(&payload.email, payload.phone.as_deref(), payload.device_id.as_deref()).await {
        Ok(user) => {
            info!("User created successfully: {}", user.id);
            Ok(HttpResponse::Ok().json(ApiResponse::success(user)))
        }
        Err(e) => {
            error!("Failed to create user: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
        }
    }
}

async fn get_user(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.user_service.get_user_by_id(path.into_inner()).await {
        Ok(Some(user)) => Ok(HttpResponse::Ok().json(ApiResponse::success(user))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("User not found".to_string()))),
        Err(e) => {
            error!("Failed to get user: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

async fn verify_user(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.user_service.verify_user(path.into_inner()).await {
        Ok(_) => {
            info!("User verified successfully: {}", path.into_inner());
            Ok(HttpResponse::Ok().json(ApiResponse::success(())))
        }
        Err(e) => {
            error!("Failed to verify user: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
        }
    }
}

async fn get_user_accounts(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.user_service.get_user_accounts(path.into_inner()).await {
        Ok(accounts) => Ok(HttpResponse::Ok().json(ApiResponse::success(accounts))),
        Err(e) => {
            error!("Failed to get user accounts: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

// ========== ACCOUNT HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateAccountRequest {
    user_id: Uuid,
    account_type: String,
    currency: String,
    initial_balance: i64,
}

async fn create_account(
    state: web::Data<AppState>,
    payload: web::Json<CreateAccountRequest>,
) -> ActixResult<HttpResponse> {
    let account_type = match payload.account_type.as_str() {
        "CHECKING" => shared::AccountType::Checking,
        "SAVINGS" => shared::AccountType::Savings,
        "CREDIT" => shared::AccountType::Credit,
        "DEBIT" => shared::AccountType::Debit,
        _ => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::error("Invalid account type".to_string())));
        }
    };

    match state.user_service.create_account(payload.user_id, account_type, &payload.currency, payload.initial_balance).await {
        Ok(account) => {
            info!("Account created successfully: {}", account.account_number);
            Ok(HttpResponse::Ok().json(ApiResponse::success(account)))
        }
        Err(e) => {
            error!("Failed to create account: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
        }
    }
}

async fn get_account(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> ActixResult<HttpResponse> {
    match state.user_service.get_account_by_number(&path.into_inner()).await {
        Ok(Some(account)) => Ok(HttpResponse::Ok().json(ApiResponse::success(account))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Account not found".to_string()))),
        Err(e) => {
            error!("Failed to get account: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

// ========== PAYMENT HANDLERS ==========

async fn transfer_money(
    state: web::Data<AppState>,
    payload: web::Json<shared::TransferRequest>,
) -> ActixResult<HttpResponse> {
    info!("Processing transfer from {} to {} for amount {}", payload.from_account, payload.to_account, payload.amount);

    if let Some(ref card_info) = payload.card_info {
        if let Err(e) = CardValidator::validate_card(card_info) {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::error(format!("Card validation failed: {}", e))));
        }

        if let Err(e) = state.fraud_detector.check_fraud(card_info, &payload.user_info) {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::error(format!("Fraud detected: {}", e))));
        }
    }

    if let Some(ref user_info) = payload.user_info {
        if !user_info.is_verified {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::error("User not verified".to_string())));
        }
    }

    match state.user_service.check_sufficient_funds(&payload.from_account, payload.amount).await {
        Ok(false) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::error("Insufficient funds".to_string())));
        }
        Err(e) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())));
        }
        Ok(true) => {}
    }

    match state.user_service.transfer_money(&payload.from_account, &payload.to_account, payload.amount).await {
        Ok(_) => {
            info!("Transfer completed successfully");
            Ok(HttpResponse::Ok().json(ApiResponse::success(())))
        }
        Err(e) => {
            error!("Failed to transfer money: {}", e);
            Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
        }
    }
}

async fn validate_card(
    state: web::Data<AppState>,
    payload: web::Json<shared::CardInfo>,
) -> ActixResult<HttpResponse> {
    let card_info = payload.into_inner();
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
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => {
            let response = serde_json::json!({
                "valid": false,
                "error": e.to_string(),
                "message": "Card validation failed"
            });
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
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
    state: web::Data<AppState>,
    payload: web::Json<VisaPaymentRequest>,
) -> ActixResult<HttpResponse> {
    info!("Processing Visa payment for amount {} {}", payload.amount, payload.currency);

    if let Err(e) = CardValidator::validate_card(&payload.card_info) {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::error(format!("Card validation failed: {}", e))));
    }

    if let Err(e) = state.visa_client.validate_card(&payload.card_info).await {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::error(format!("Visa card validation failed: {}", e))));
    }

    if let Err(e) = state.fraud_detector.check_fraud(&payload.card_info, &None) {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::error(format!("Fraud detected: {}", e))));
    }

    match state.visa_client.process_payment(&payload.card_info, payload.amount, &payload.currency).await {
        Ok(visa_response) => {
            info!("Visa payment successful: {}", visa_response.transaction_id);
            
            let card_type_info = state.visa_client.get_card_type_info(&payload.card_info.pan).await.unwrap_or_default();
            
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
                "masked_card": CardValidator::mask_card_number(&payload.card_info.pan),
                "card_type_info": card_type_info,
            });
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Visa payment failed: {}", e);
            let response = serde_json::json!({
                "success": false,
                "error": e.to_string(),
                "card_type": CardValidator::get_card_type(&payload.card_info.pan),
                "masked_card": CardValidator::mask_card_number(&payload.card_info.pan)
            });
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
    }
}

// ========== REFUND HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateRefundRequest {
    amount: Option<i64>,
    reason: Option<String>,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_refund(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<CreateRefundRequest>,
) -> ActixResult<HttpResponse> {
    let reason = payload.reason.as_deref().and_then(|r| match r {
        "requested_by_customer" => Some(shared::RefundReason::RequestedByCustomer),
        "duplicate" => Some(shared::RefundReason::Duplicate),
        "fraudulent" => Some(shared::RefundReason::Fraudulent),
        "other" => Some(shared::RefundReason::Other),
        _ => None,
    });

    match state.refund_service.create_refund(path.into_inner(), payload.amount, reason, payload.metadata.clone()).await {
        Ok(refund) => {
            let _ = state.webhook_service.deliver_event(
                "refund.created".to_string(),
                serde_json::to_value(&refund).unwrap(),
            ).await;
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(refund)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_refund(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.refund_service.get_refund(path.into_inner()).await {
        Ok(Some(refund)) => Ok(HttpResponse::Ok().json(ApiResponse::success(refund))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Refund not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn list_refunds(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.refund_service.list_refunds_for_transaction(path.into_inner()).await {
        Ok(refunds) => Ok(HttpResponse::Ok().json(ApiResponse::success(refunds))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

// ========== CUSTOMER HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateCustomerRequest {
    email: Option<String>,
    phone: Option<String>,
    name: Option<String>,
    description: Option<String>,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_customer(
    state: web::Data<AppState>,
    payload: web::Json<CreateCustomerRequest>,
) -> ActixResult<HttpResponse> {
    match state.customer_service.create_customer(
        payload.email.clone(),
        payload.phone.clone(),
        payload.name.clone(),
        payload.description.clone(),
        payload.metadata.clone(),
    ).await {
        Ok(customer) => {
            let _ = state.webhook_service.deliver_event(
                "customer.created".to_string(),
                serde_json::to_value(&customer).unwrap(),
            ).await;
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(customer)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_customer(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.customer_service.get_customer(path.into_inner()).await {
        Ok(Some(customer)) => Ok(HttpResponse::Ok().json(ApiResponse::success(customer))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Customer not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn update_customer(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<CreateCustomerRequest>,
) -> ActixResult<HttpResponse> {
    match state.customer_service.update_customer(
        path.into_inner(),
        payload.email.clone(),
        payload.phone.clone(),
        payload.name.clone(),
        payload.description.clone(),
        payload.metadata.clone(),
    ).await {
        Ok(customer) => Ok(HttpResponse::Ok().json(ApiResponse::success(customer))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn list_customers(
    state: web::Data<AppState>,
    query: web::Query<ListQuery>,
) -> ActixResult<HttpResponse> {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);
    
    match state.customer_service.list_customers(limit, offset).await {
        Ok(customers) => Ok(HttpResponse::Ok().json(ApiResponse::success(customers))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn delete_customer(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.customer_service.delete_customer(path.into_inner()).await {
        Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse::success(()))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

// ========== PAYMENT METHOD HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreatePaymentMethodRequest {
    customer_id: Option<Uuid>,
    card_info: shared::CardInfo,
    r#type: String,
    is_default: Option<bool>,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_payment_method(
    state: web::Data<AppState>,
    payload: web::Json<CreatePaymentMethodRequest>,
) -> ActixResult<HttpResponse> {
    let r#type = match payload.r#type.as_str() {
        "card" => shared::PaymentMethodType::Card,
        "ach" => shared::PaymentMethodType::Ach,
        "bank_account" => shared::PaymentMethodType::BankAccount,
        "paypal" => shared::PaymentMethodType::Paypal,
        "apple_pay" => shared::PaymentMethodType::ApplePay,
        "google_pay" => shared::PaymentMethodType::GooglePay,
        _ => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::error("Invalid payment method type".to_string())));
        }
    };

    if let Err(e) = CardValidator::validate_card(&payload.card_info) {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::error(format!("Card validation failed: {}", e))));
    }

    match state.payment_method_service.create_payment_method(
        payload.customer_id,
        &payload.card_info,
        r#type,
        payload.is_default,
        payload.metadata.clone(),
    ).await {
        Ok(pm) => {
            let _ = state.webhook_service.deliver_event(
                "payment_method.created".to_string(),
                serde_json::to_value(&pm).unwrap(),
            ).await;
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(pm)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_payment_method(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.payment_method_service.get_payment_method(path.into_inner()).await {
        Ok(Some(pm)) => Ok(HttpResponse::Ok().json(ApiResponse::success(pm))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Payment method not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(Deserialize)]
struct PaymentMethodQuery {
    customer_id: Option<Uuid>,
}

async fn list_payment_methods(
    state: web::Data<AppState>,
    query: web::Query<PaymentMethodQuery>,
) -> ActixResult<HttpResponse> {
    match state.payment_method_service.list_payment_methods(query.customer_id).await {
        Ok(pms) => Ok(HttpResponse::Ok().json(ApiResponse::success(pms))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn delete_payment_method(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.payment_method_service.delete_payment_method(path.into_inner()).await {
        Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse::success(()))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn set_default_payment_method(
    state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
) -> ActixResult<HttpResponse> {
    let (customer_id, payment_method_id) = path.into_inner();
    match state.payment_method_service.set_default_payment_method(customer_id, payment_method_id).await {
        Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse::success(()))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

// ========== WEBHOOK HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateWebhookRequest {
    url: String,
    events: Vec<String>,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_webhook(
    state: web::Data<AppState>,
    payload: web::Json<CreateWebhookRequest>,
) -> ActixResult<HttpResponse> {
    match state.webhook_service.create_webhook(payload.url.clone(), payload.events.clone(), payload.metadata.clone()).await {
        Ok(webhook) => Ok(HttpResponse::Ok().json(ApiResponse::success(webhook))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_webhook(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.webhook_service.get_webhook(path.into_inner()).await {
        Ok(Some(webhook)) => Ok(HttpResponse::Ok().json(ApiResponse::success(webhook))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Webhook not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn list_webhooks(
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    match state.webhook_service.list_webhooks().await {
        Ok(webhooks) => Ok(HttpResponse::Ok().json(ApiResponse::success(webhooks))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn handle_webhook_delivery(
    state: web::Data<AppState>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(ApiResponse::success(())))
}

// ========== SUBSCRIPTION HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateSubscriptionRequest {
    customer_id: Uuid,
    price_id: Uuid,
    trial_days: Option<u32>,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_subscription(
    state: web::Data<AppState>,
    payload: web::Json<CreateSubscriptionRequest>,
) -> ActixResult<HttpResponse> {
    match state.subscription_service.create_subscription(
        payload.customer_id,
        payload.price_id,
        payload.trial_days,
        payload.metadata.clone(),
    ).await {
        Ok(subscription) => {
            let _ = state.webhook_service.deliver_event(
                "subscription.created".to_string(),
                serde_json::to_value(&subscription).unwrap(),
            ).await;
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(subscription)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_subscription(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.subscription_service.get_subscription(path.into_inner()).await {
        Ok(Some(subscription)) => Ok(HttpResponse::Ok().json(ApiResponse::success(subscription))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Subscription not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(Deserialize)]
struct SubscriptionQuery {
    customer_id: Option<Uuid>,
    status: Option<String>,
}

async fn list_subscriptions(
    state: web::Data<AppState>,
    query: web::Query<SubscriptionQuery>,
) -> ActixResult<HttpResponse> {
    let status = query.status.as_deref().and_then(|s| match s {
        "INCOMPLETE" => Some(shared::SubscriptionStatus::Incomplete),
        "ACTIVE" => Some(shared::SubscriptionStatus::Active),
        "CANCELED" => Some(shared::SubscriptionStatus::Canceled),
        "TRIALING" => Some(shared::SubscriptionStatus::Trialing),
        _ => None,
    });
    
    match state.subscription_service.list_subscriptions(query.customer_id, status).await {
        Ok(subscriptions) => Ok(HttpResponse::Ok().json(ApiResponse::success(subscriptions))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn cancel_subscription(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let at_period_end = payload.get("at_period_end")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    
    match state.subscription_service.cancel_subscription(path.into_inner(), at_period_end).await {
        Ok(subscription) => {
            let _ = state.webhook_service.deliver_event(
                "subscription.canceled".to_string(),
                serde_json::to_value(&subscription).unwrap(),
            ).await;
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(subscription)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

// ========== DISPUTE HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateDisputeRequest {
    transaction_id: Uuid,
    reason: Option<String>,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_dispute(
    state: web::Data<AppState>,
    payload: web::Json<CreateDisputeRequest>,
) -> ActixResult<HttpResponse> {
    let reason = payload.reason.as_deref().and_then(|r| match r {
        "FRAUDULENT" => Some(shared::DisputeReason::Fraudulent),
        "DUPLICATE" => Some(shared::DisputeReason::Duplicate),
        "CUSTOMER_INITIATED" => Some(shared::DisputeReason::CustomerInitiated),
        _ => None,
    });

    match state.dispute_service.create_dispute(payload.transaction_id, reason, payload.metadata.clone()).await {
        Ok(dispute) => {
            let _ = state.webhook_service.deliver_event(
                "dispute.created".to_string(),
                serde_json::to_value(&dispute).unwrap(),
            ).await;
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(dispute)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_dispute(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.dispute_service.get_dispute(path.into_inner()).await {
        Ok(Some(dispute)) => Ok(HttpResponse::Ok().json(ApiResponse::success(dispute))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Dispute not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn list_disputes(
    state: web::Data<AppState>,
    query: web::Query<DisputeQuery>,
) -> ActixResult<HttpResponse> {
    let status = query.status.as_deref().and_then(|s| match s {
        "NEEDS_RESPONSE" => Some(shared::DisputeStatus::NeedsResponse),
        "UNDER_REVIEW" => Some(shared::DisputeStatus::UnderReview),
        "WON" => Some(shared::DisputeStatus::Won),
        "LOST" => Some(shared::DisputeStatus::Lost),
        _ => None,
    });
    
    match state.dispute_service.list_disputes(status, query.limit.unwrap_or(100), query.offset.unwrap_or(0)).await {
        Ok(disputes) => Ok(HttpResponse::Ok().json(ApiResponse::success(disputes))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(Deserialize)]
struct DisputeQuery {
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn submit_dispute_evidence(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let evidence_type = payload.get("evidence_type").and_then(|v| v.as_str()).unwrap_or("document");
    let evidence_data = payload.get("evidence_data").cloned().unwrap_or_default();

    match state.dispute_service.submit_evidence(path.into_inner(), evidence_type.to_string(), evidence_data).await {
        Ok(dispute) => Ok(HttpResponse::Ok().json(ApiResponse::success(dispute))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn update_dispute_status(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let status_str = payload.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let status = match status_str {
        "WON" => shared::DisputeStatus::Won,
        "LOST" => shared::DisputeStatus::Lost,
        "CHARGE_REFUNDED" => shared::DisputeStatus::ChargeRefunded,
        _ => return Ok(HttpResponse::BadRequest().json(ApiResponse::error("Invalid status".to_string()))),
    };

    match state.dispute_service.update_dispute_status(path.into_inner(), status).await {
        Ok(dispute) => Ok(HttpResponse::Ok().json(ApiResponse::success(dispute))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

// ========== INVOICE HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateInvoiceRequest {
    customer_id: Uuid,
    subscription_id: Option<Uuid>,
    line_items: Vec<invoicing::InvoiceLineItemInput>,
    due_date: Option<chrono::DateTime<chrono::Utc>>,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_invoice(
    state: web::Data<AppState>,
    payload: web::Json<CreateInvoiceRequest>,
) -> ActixResult<HttpResponse> {
    match state.invoice_service.create_invoice(
        payload.customer_id,
        payload.subscription_id,
        payload.line_items.clone(),
        payload.due_date,
        payload.metadata.clone(),
    ).await {
        Ok(invoice) => {
            let _ = state.webhook_service.deliver_event(
                "invoice.created".to_string(),
                serde_json::to_value(&invoice).unwrap(),
            ).await;
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(invoice)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_invoice(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.invoice_service.get_invoice(path.into_inner()).await {
        Ok(Some(invoice)) => Ok(HttpResponse::Ok().json(ApiResponse::success(invoice))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Invoice not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn list_invoices(
    state: web::Data<AppState>,
    query: web::Query<InvoiceQuery>,
) -> ActixResult<HttpResponse> {
    let status = query.status.as_deref().and_then(|s| match s {
        "DRAFT" => Some(shared::InvoiceStatus::Draft),
        "OPEN" => Some(shared::InvoiceStatus::Open),
        "PAID" => Some(shared::InvoiceStatus::Paid),
        _ => None,
    });
    
    match state.invoice_service.list_invoices(query.customer_id, status, query.limit.unwrap_or(100), query.offset.unwrap_or(0)).await {
        Ok(invoices) => Ok(HttpResponse::Ok().json(ApiResponse::success(invoices))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(Deserialize)]
struct InvoiceQuery {
    customer_id: Option<Uuid>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn finalize_invoice(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.invoice_service.finalize_invoice(path.into_inner()).await {
        Ok(invoice) => Ok(HttpResponse::Ok().json(ApiResponse::success(invoice))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn mark_invoice_paid(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let amount = payload.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
    
    match state.invoice_service.mark_invoice_paid(path.into_inner(), amount).await {
        Ok(invoice) => Ok(HttpResponse::Ok().json(ApiResponse::success(invoice))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_invoice_line_items(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.invoice_service.get_invoice_line_items(path.into_inner()).await {
        Ok(items) => Ok(HttpResponse::Ok().json(ApiResponse::success(items))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

// ========== PAYOUT HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreatePayoutRequest {
    account_id: Uuid,
    amount: i64,
    currency: String,
    payout_method: String,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_payout(
    state: web::Data<AppState>,
    payload: web::Json<CreatePayoutRequest>,
) -> ActixResult<HttpResponse> {
    let payout_method = match payload.payout_method.as_str() {
        "bank_account" => shared::PayoutMethod::BankAccount,
        "card" => shared::PayoutMethod::Card,
        "instant" => shared::PayoutMethod::Instant,
        _ => return Ok(HttpResponse::BadRequest().json(ApiResponse::error("Invalid payout method".to_string()))),
    };

    match state.payout_service.create_payout(
        payload.account_id,
        payload.amount,
        payload.currency.clone(),
        payout_method,
        None,
        payload.metadata.clone(),
    ).await {
        Ok(payout) => {
            let _ = state.webhook_service.deliver_event(
                "payout.created".to_string(),
                serde_json::to_value(&payout).unwrap(),
            ).await;
            
            Ok(HttpResponse::Ok().json(ApiResponse::success(payout)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_payout(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.payout_service.get_payout(path.into_inner()).await {
        Ok(Some(payout)) => Ok(HttpResponse::Ok().json(ApiResponse::success(payout))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Payout not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn list_payouts(
    state: web::Data<AppState>,
    query: web::Query<PayoutQuery>,
) -> ActixResult<HttpResponse> {
    let status = query.status.as_deref().and_then(|s| match s {
        "PENDING" => Some(shared::PayoutStatus::Pending),
        "PAID" => Some(shared::PayoutStatus::Paid),
        "FAILED" => Some(shared::PayoutStatus::Failed),
        _ => None,
    });
    
    match state.payout_service.list_payouts(query.account_id, status, query.limit.unwrap_or(100), query.offset.unwrap_or(0)).await {
        Ok(payouts) => Ok(HttpResponse::Ok().json(ApiResponse::success(payouts))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(Deserialize)]
struct PayoutQuery {
    account_id: Option<Uuid>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn cancel_payout(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.payout_service.cancel_payout(path.into_inner()).await {
        Ok(payout) => Ok(HttpResponse::Ok().json(ApiResponse::success(payout))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

// ========== CONNECT/MARKETPLACE HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreateConnectAccountRequest {
    email: Option<String>,
    country: String,
    account_type: String,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_connect_account(
    state: web::Data<AppState>,
    payload: web::Json<CreateConnectAccountRequest>,
) -> ActixResult<HttpResponse> {
    let account_type = match payload.account_type.as_str() {
        "express" => shared::ConnectAccountType::Express,
        "standard" => shared::ConnectAccountType::Standard,
        "custom" => shared::ConnectAccountType::Custom,
        _ => return Ok(HttpResponse::BadRequest().json(ApiResponse::error("Invalid account type".to_string()))),
    };

    match state.connect_service.create_connect_account(
        payload.email.clone(),
        payload.country.clone(),
        account_type,
        payload.metadata.clone(),
    ).await {
        Ok(account) => Ok(HttpResponse::Ok().json(ApiResponse::success(account))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_connect_account(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.connect_service.get_connect_account(path.into_inner()).await {
        Ok(Some(account)) => Ok(HttpResponse::Ok().json(ApiResponse::success(account))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Connect account not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn update_connect_account(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let charges_enabled = payload.get("charges_enabled").and_then(|v| v.as_bool());
    let payouts_enabled = payload.get("payouts_enabled").and_then(|v| v.as_bool());
    let details_submitted = payload.get("details_submitted").and_then(|v| v.as_bool());

    match state.connect_service.update_account_status(path.into_inner(), charges_enabled, payouts_enabled, details_submitted).await {
        Ok(account) => Ok(HttpResponse::Ok().json(ApiResponse::success(account))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(serde::Deserialize)]
struct CreateTransferRequest {
    transaction_id: Uuid,
    destination_account_id: Uuid,
    amount: i64,
    currency: String,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_transfer(
    state: web::Data<AppState>,
    payload: web::Json<CreateTransferRequest>,
) -> ActixResult<HttpResponse> {
    match state.connect_service.create_transfer(
        payload.transaction_id,
        payload.destination_account_id,
        payload.amount,
        payload.currency.clone(),
        payload.metadata.clone(),
    ).await {
        Ok(transfer) => Ok(HttpResponse::Ok().json(ApiResponse::success(transfer))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn list_transfers(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.connect_service.list_transfers_for_transaction(path.into_inner()).await {
        Ok(transfers) => Ok(HttpResponse::Ok().json(ApiResponse::success(transfers))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

// ========== PAYMENT INTENT HANDLERS ==========

#[derive(serde::Deserialize)]
struct CreatePaymentIntentRequest {
    customer_id: Option<Uuid>,
    payment_method_id: Option<Uuid>,
    amount: i64,
    currency: String,
    confirmation_method: Option<String>,
    metadata: Option<std::collections::HashMap<String, String>>,
}

async fn create_payment_intent(
    state: web::Data<AppState>,
    payload: web::Json<CreatePaymentIntentRequest>,
) -> ActixResult<HttpResponse> {
    let confirmation_method = payload.confirmation_method.as_deref()
        .map(|s| if s == "manual" { shared::ConfirmationMethod::Manual } else { shared::ConfirmationMethod::Automatic })
        .unwrap_or(shared::ConfirmationMethod::Automatic);

    match state.payment_intent_service.create_payment_intent(
        payload.customer_id,
        payload.payment_method_id,
        payload.amount,
        payload.currency.clone(),
        confirmation_method,
        payload.metadata.clone(),
    ).await {
        Ok(intent) => Ok(HttpResponse::Ok().json(ApiResponse::success(intent))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_payment_intent(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.payment_intent_service.get_payment_intent(path.into_inner()).await {
        Ok(Some(intent)) => Ok(HttpResponse::Ok().json(ApiResponse::success(intent))),
        Ok(None) => Ok(HttpResponse::NotFound().json(ApiResponse::error("Payment Intent not found".to_string()))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

async fn confirm_payment_intent(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let payment_method_id = payload.get("payment_method_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    match state.payment_intent_service.confirm_payment_intent(path.into_inner(), payment_method_id).await {
        Ok(intent) => Ok(HttpResponse::Ok().json(ApiResponse::success(intent))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn cancel_payment_intent(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> ActixResult<HttpResponse> {
    match state.payment_intent_service.cancel_payment_intent(path.into_inner()).await {
        Ok(intent) => Ok(HttpResponse::Ok().json(ApiResponse::success(intent))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn handle_3d_secure(
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let authentication_result = payload.get("authentication_result")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    match state.payment_intent_service.handle_3d_secure(path.into_inner(), authentication_result).await {
        Ok(intent) => Ok(HttpResponse::Ok().json(ApiResponse::success(intent))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

// ========== CURRENCY HANDLERS ==========

#[derive(serde::Deserialize)]
struct ConvertCurrencyRequest {
    amount: i64,
    from_currency: String,
    to_currency: String,
}

async fn convert_currency(
    state: web::Data<AppState>,
    payload: web::Json<ConvertCurrencyRequest>,
) -> ActixResult<HttpResponse> {
    match state.currency_service.convert_currency(
        payload.amount,
        &payload.from_currency,
        &payload.to_currency,
    ).await {
        Ok(converted_amount) => {
            let response = serde_json::json!({
                "original_amount": payload.amount,
                "original_currency": payload.from_currency,
                "converted_amount": converted_amount,
                "target_currency": payload.to_currency,
            });
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_exchange_rate(
    state: web::Data<AppState>,
    query: web::Query<ExchangeRateQuery>,
) -> ActixResult<HttpResponse> {
    match state.currency_service.get_exchange_rate(&query.base_currency, &query.target_currency).await {
        Ok(rate) => Ok(HttpResponse::Ok().json(ApiResponse::success(rate))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(Deserialize)]
struct ExchangeRateQuery {
    base_currency: String,
    target_currency: String,
}

async fn set_exchange_rate(
    state: web::Data<AppState>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let base_currency = payload.get("base_currency").and_then(|v| v.as_str()).unwrap_or("");
    let target_currency = payload.get("target_currency").and_then(|v| v.as_str()).unwrap_or("");
    let rate = payload.get("rate").and_then(|v| v.as_f64()).unwrap_or(1.0);

    match state.currency_service.set_exchange_rate(base_currency, target_currency, rate, None).await {
        Ok(exchange_rate) => Ok(HttpResponse::Ok().json(ApiResponse::success(exchange_rate))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn get_supported_currencies(
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(ApiResponse::success(currency::CurrencyService::get_supported_currencies())))
}

// ========== TAX HANDLERS ==========

#[derive(serde::Deserialize)]
struct CalculateTaxRequest {
    amount: i64,
    country: Option<String>,
    jurisdiction: Option<String>,
}

async fn calculate_tax(
    state: web::Data<AppState>,
    payload: web::Json<CalculateTaxRequest>,
) -> ActixResult<HttpResponse> {
    match state.tax_service.calculate_tax(
        payload.amount,
        payload.country.as_deref(),
        payload.jurisdiction.as_deref(),
    ).await {
        Ok(calculation) => {
            let response = serde_json::json!({
                "subtotal": calculation.subtotal,
                "tax_amount": calculation.tax_amount,
                "total": calculation.total,
                "tax_rate": {
                    "percentage": calculation.tax_rate.percentage,
                    "display_name": calculation.tax_rate.display_name,
                },
            });
            Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn create_tax_rate(
    state: web::Data<AppState>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let display_name = payload.get("display_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let percentage = payload.get("percentage").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let inclusive = payload.get("inclusive").and_then(|v| v.as_bool()).unwrap_or(false);
    let country = payload.get("country").and_then(|v| v.as_str()).map(|s| s.to_string());
    let jurisdiction = payload.get("jurisdiction").and_then(|v| v.as_str()).map(|s| s.to_string());

    match state.tax_service.create_tax_rate(display_name, percentage, inclusive, country, jurisdiction, None).await {
        Ok(tax_rate) => Ok(HttpResponse::Ok().json(ApiResponse::success(tax_rate))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(e.to_string())))
    }
}

async fn list_tax_rates(
    state: web::Data<AppState>,
    query: web::Query<TaxRateQuery>,
) -> ActixResult<HttpResponse> {
    match state.tax_service.list_tax_rates(query.country.as_deref(), query.active_only.unwrap_or(true)).await {
        Ok(rates) => Ok(HttpResponse::Ok().json(ApiResponse::success(rates))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::error(e.to_string())))
    }
}

#[derive(Deserialize)]
struct TaxRateQuery {
    country: Option<String>,
    active_only: Option<bool>,
}

// ========== TEST MODE HANDLERS ==========

async fn get_test_mode_status(
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let enabled = state.test_mode_service.is_test_mode_enabled().await;
    let response = serde_json::json!({
        "test_mode_enabled": enabled,
    });
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

async fn enable_test_mode(
    state: web::Data<AppState>,
    payload: web::Json<serde_json::Value>,
) -> ActixResult<HttpResponse> {
    let enabled = payload.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    match state.test_mode_service.set_test_mode(enabled).await {
        Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse::success(()))),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::error(format!("Failed to set test mode: {}", e))))
    }
}

async fn get_test_cards(
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let cards = state.test_mode_service.get_test_cards().await;
    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::to_value(cards).unwrap())))
}
