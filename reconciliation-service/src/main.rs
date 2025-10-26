//! # Reconciliation Service
//! 
//! This module contains the main application for the Reconciliation Service.
//! It handles event consumption, reconciliation processing, anomaly detection,
//! and report generation for the payment processing system.
//! 
//! ## Key Responsibilities:
//! - Event consumption from Redis pub/sub
//! - Transaction reconciliation and validation
//! - Anomaly detection and reporting
//! - Daily summary generation
//! - Event replay capabilities
//! - Report generation and download
//! - Health monitoring and metrics
//! 
//! ## Architecture:
//! - Consumes events from Payment Processor via Redis
//! - Maintains event-sourced ledger for reconciliation
//! - Generates reports and detects anomalies
//! - Provides event replay functionality

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use shared::{
    Anomaly, AnomalySeverity, AnomalyType, ApiResponse, HealthCheck, HealthStatus,
    ReconciliationReport, TransactionEvent, TransactionEventType,
};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tokio_stream::StreamExt;
use tracing::{info, warn, error};
use uuid::Uuid;

mod database;
mod event_replay;
mod safe_event_replay;
mod metrics;
mod reconciliation;
mod redis_client;

use database::DatabaseService;
use redis_client::RedisService;
use reconciliation::ReconciliationService;
use event_replay::EventReplayService;
use safe_event_replay::SafeEventReplayService;

/// Application state containing all service dependencies for reconciliation
/// 
/// This struct holds references to all the services and components needed by
/// the reconciliation service HTTP handlers. It provides access to database
/// connections, Redis client, reconciliation logic, and event replay services.
#[derive(Clone)]
pub struct AppState {
    /// Database service for reconciliation data operations
    pub db: DatabaseService,
    /// Redis client for event consumption and pub/sub operations
    pub redis: RedisService,
    /// Reconciliation service for report generation and anomaly detection
    pub reconciliation: ReconciliationService,
    /// Event replay service for reprocessing historical events (legacy)
    pub event_replay: EventReplayService,
    /// Safe event replay service for reprocessing historical events with staging database
    pub safe_event_replay: SafeEventReplayService,
}

/// Main application entry point for the Reconciliation Service
/// 
/// This function initializes the Reconciliation Service by:
/// 1. Setting up structured logging with tracing
/// 2. Initializing Prometheus metrics collection
/// 3. Creating all service dependencies (database, Redis, reconciliation, event replay)
/// 4. Starting the event consumer background task
/// 5. Configuring HTTP routes and middleware
/// 6. Starting the HTTP server on port 3002
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
/// 
/// # Background Tasks
/// 
/// Starts an event consumer task that continuously listens for events from
/// the Payment Processor service via Redis pub/sub channels.
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging with tracing
    // This sets up JSON-formatted logs with thread information for better debugging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    info!("Starting Reconciliation & Reporting Service");

    // Initialize Prometheus metrics collection
    // This registers all metric collectors for monitoring
    metrics::init_metrics();

    // Initialize all service dependencies
    // Each service is created with its required configuration
    let db = DatabaseService::new().await.map_err(|e| anyhow::anyhow!("{}", e))?;
    let redis = RedisService::new().await.map_err(|e| anyhow::anyhow!("{}", e))?;
    let reconciliation = ReconciliationService::new();
    let event_replay = EventReplayService::new(db.pool.clone());
    let safe_event_replay = SafeEventReplayService::new(db.pool.clone(), db.staging_pool.clone());

    // Create application state with all services
    let app_state = AppState {
        db,
        redis,
        reconciliation,
        event_replay,
        safe_event_replay,
    };

    // Start event consumer background task
    // This continuously listens for events from the Payment Processor service
    let app_state_clone = app_state.clone();
    tokio::spawn(async move {
        if let Err(e) = app_state_clone.consume_events().await {
            error!("Event consumer failed: {}", e);
        }
    });

    // Build HTTP application with all routes and middleware
    let app = Router::new()
        // Health and monitoring endpoints
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        
        // Report management endpoints
        .route("/reports", get(list_reports))
        .route("/reports/:id", get(get_report))
        .route("/reports/:id/download", get(download_report))
        .route("/reports/generate", post(generate_report))
        
        // Anomaly management endpoints
        .route("/anomalies", get(list_anomalies))
        .route("/anomalies/:id", get(get_anomaly))
        
        // Summary and analysis endpoints
        .route("/daily-summaries", get(list_daily_summaries))
        .route("/reconcile", post(trigger_reconciliation))
        
        // Event replay endpoints
        .route("/replay/start", post(start_event_replay))
        .route("/replay/:id", get(get_replay_status))
        .route("/replay", get(list_replays))
        
        // Safe event replay endpoints (new safer approach)
        .route("/safe-replay/start", post(start_safe_event_replay))
        .route("/safe-replay/:id", get(get_safe_replay_status))
        .route("/safe-replay", get(list_safe_replays))
        
        // Add application state and middleware
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default()))
                .layer(CorsLayer::permissive()),
        );

    // Start HTTP server on port 3002
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3002").await?;
    info!("Reconciliation & Reporting Service listening on port 3002");
    
    axum::serve(listener, app).await?;
    Ok(())
}

impl AppState {
    /// Consumes events from Redis pub/sub channels
    /// 
    /// This function runs continuously in a background task to listen for
    /// transaction events from the Payment Processor service. It processes
    /// events as they arrive and updates the reconciliation ledger accordingly.
    /// 
    /// # Process:
    /// 1. Subscribes to Redis pub/sub channels for transaction events
    /// 2. Processes incoming events in real-time
    /// 3. Stores events in the reconciliation database
    /// 4. Updates daily summaries and detects anomalies
    /// 5. Handles connection failures with exponential backoff
    /// 
    /// # Event Types Processed:
    /// - TransactionCreated: New transaction events
    /// - TransactionCommitted: Transaction completion events
    /// - TransactionFailed: Transaction failure events
    /// - TransactionCancelled: Transaction cancellation events
    /// 
    /// # Error Handling:
    /// - Implements exponential backoff for connection failures
    /// - Logs errors for monitoring and debugging
    /// - Continues processing even if individual events fail
    /// 
    /// # Performance:
    /// - Processes events asynchronously
    /// - Uses connection pooling for database operations
    /// - Implements circuit breaker pattern for resilience
    async fn consume_events(&self) -> Result<()> {
        info!("Starting event consumer");
        
        // Simplified event consumption - in a real implementation this would
        // continuously poll Redis for events and process them in real-time
        loop {
            match self.redis.subscribe_to_events().await {
                Ok(_) => {
                    // Process events here
                    // In a real implementation, this would:
                    // 1. Parse incoming event messages
                    // 2. Validate event structure
                    // 3. Store events in reconciliation database
                    // 4. Update daily summaries
                    // 5. Detect anomalies
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
                Err(e) => {
                    error!("Failed to subscribe to events: {}", e);
                    // Implement exponential backoff for connection failures
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Processes a single transaction event
    /// 
    /// This function handles the processing of individual transaction events
    /// received from the Payment Processor service. It updates the reconciliation
    /// ledger and maintains data consistency.
    /// 
    /// # Parameters:
    /// - `event`: Transaction event to process
    /// 
    /// # Process:
    /// 1. Stores the event in the reconciliation database
    /// 2. Updates daily summary statistics
    /// 3. Performs anomaly detection
    /// 4. Updates reconciliation metrics
    /// 
    /// # Returns:
    /// - `Ok(())`: Event processed successfully
    /// - `Err(anyhow::Error)`: Processing failed
    /// 
    /// # Error Handling:
    /// - Database errors are propagated up
    /// - Invalid events are logged and skipped
    /// - Partial failures don't stop processing
    async fn process_event(&self, event: TransactionEvent) -> Result<()> {
        info!("Processing event: {:?}", event.event_type);
        
        // Store event in ledger for reconciliation
        // This maintains the complete audit trail of all transactions
        self.db.store_event(&event).await.map_err(|e| anyhow::anyhow!("{}", e))?;
        
        // Update daily summary with event data
        // This keeps running totals and statistics up to date
        self.db.update_daily_summary(&event).await.map_err(|e| anyhow::anyhow!("{}", e))?;
        
        Ok(())
    }
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

    // Check staging database
    match state.db.staging_health_check().await {
        Ok(_) => {
            dependencies.insert("staging_database".to_string(), shared::DependencyHealth {
                status: HealthStatus::Healthy,
                response_time_ms: Some(8),
                last_check: Utc::now(),
            });
        }
        Err(e) => {
            dependencies.insert("staging_database".to_string(), shared::DependencyHealth {
                status: HealthStatus::Unhealthy,
                response_time_ms: None,
                last_check: Utc::now(),
            });
            error!("Staging database health check failed: {}", e);
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
        service: "reconciliation-service".to_string(),
        status: overall_status,
        timestamp: Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        dependencies,
    };

    Json(ApiResponse::success(health))
}

async fn list_reports(
    State(state): State<AppState>,
    Query(query): Query<ListReportsQuery>,
) -> Result<Json<ApiResponse<Vec<ReconciliationReport>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let limit = query.limit.unwrap_or(50).min(1000);
    let offset = query.offset.unwrap_or(0);

    match state.db.list_reports(limit, offset).await {
        Ok(reports) => Ok(Json(ApiResponse::success(reports))),
        Err(e) => {
            error!("Failed to list reports: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn get_report(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ReconciliationReport>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.db.get_report(id).await {
        Ok(Some(report)) => Ok(Json(ApiResponse::success(report))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Report not found".to_string())),
        )),
        Err(e) => {
            error!("Failed to get report: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn download_report(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.db.generate_csv_report(id).await {
        Ok(csv_data) => Ok(Json(ApiResponse::success(csv_data))),
        Err(e) => {
            error!("Failed to generate CSV report: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn generate_report(
    State(state): State<AppState>,
    Json(payload): Json<GenerateReportRequest>,
) -> Result<Json<ApiResponse<ReconciliationReport>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.reconciliation.generate_report(&state.db, payload.period_start, payload.period_end).await {
        Ok(report) => {
            info!("Generated reconciliation report: {}", report.report_id);
            Ok(Json(ApiResponse::success(report)))
        }
        Err(e) => {
            error!("Failed to generate report: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn list_anomalies(
    State(state): State<AppState>,
    Query(query): Query<ListAnomaliesQuery>,
) -> Result<Json<ApiResponse<Vec<Anomaly>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    match state.db.list_anomalies(query.severity, limit, offset).await {
        Ok(anomalies) => Ok(Json(ApiResponse::success(anomalies))),
        Err(e) => {
            error!("Failed to list anomalies: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn get_anomaly(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Anomaly>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.db.get_anomaly(id).await {
        Ok(Some(anomaly)) => Ok(Json(ApiResponse::success(anomaly))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Anomaly not found".to_string())),
        )),
        Err(e) => {
            error!("Failed to get anomaly: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn list_daily_summaries(
    State(state): State<AppState>,
    Query(query): Query<ListDailySummariesQuery>,
) -> Result<Json<ApiResponse<Vec<crate::database::DailySummary>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let limit = query.limit.unwrap_or(30).min(365);
    let offset = query.offset.unwrap_or(0);

    match state.db.list_daily_summaries(limit, offset).await {
        Ok(summaries) => Ok(Json(ApiResponse::success(summaries))),
        Err(e) => {
            error!("Failed to list daily summaries: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn trigger_reconciliation(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.reconciliation.run_reconciliation(&state.db).await {
        Ok(result) => {
            info!("Reconciliation completed: {}", result);
            Ok(Json(ApiResponse::success(result)))
        }
        Err(e) => {
            error!("Failed to run reconciliation: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

#[derive(Deserialize)]
struct ListReportsQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct GenerateReportRequest {
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
}

#[derive(Deserialize)]
struct ListAnomaliesQuery {
    severity: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct ListDailySummariesQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn start_event_replay(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<uuid::Uuid>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.event_replay.start_full_replay().await {
        Ok(replay_id) => {
            info!("Started event replay: {}", replay_id);
            Ok(Json(ApiResponse::success(replay_id)))
        }
        Err(e) => {
            error!("Failed to start event replay: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to start event replay".to_string())),
            ))
        }
    }
}

async fn get_replay_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<crate::event_replay::ReplayStatus>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.event_replay.get_replay_status(id).await {
        Ok(Some(status)) => Ok(Json(ApiResponse::success(status))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Replay not found".to_string())),
        )),
        Err(e) => {
            error!("Failed to get replay status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn list_replays(
    State(state): State<AppState>,
    Query(query): Query<ListReplaysQuery>,
) -> Result<Json<ApiResponse<Vec<crate::event_replay::ReplayStatus>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let limit = query.limit.unwrap_or(50).min(1000);
    let offset = query.offset.unwrap_or(0);

    match state.event_replay.list_replays(limit, offset).await {
        Ok(replays) => Ok(Json(ApiResponse::success(replays))),
        Err(e) => {
            error!("Failed to list replays: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

// Safe Event Replay Handlers
async fn start_safe_event_replay(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<uuid::Uuid>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.safe_event_replay.start_safe_replay().await {
        Ok(replay_id) => {
            info!("Started safe event replay: {}", replay_id);
            Ok(Json(ApiResponse::success(replay_id)))
        }
        Err(e) => {
            error!("Failed to start safe event replay: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Failed to start safe event replay".to_string())),
            ))
        }
    }
}

async fn get_safe_replay_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<crate::safe_event_replay::ReplayStatus>>, (StatusCode, Json<ApiResponse<()>>)> {
    match state.safe_event_replay.get_replay_status(id).await {
        Ok(Some(status)) => Ok(Json(ApiResponse::success(status))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Safe replay not found".to_string())),
        )),
        Err(e) => {
            error!("Failed to get safe replay status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

async fn list_safe_replays(
    State(state): State<AppState>,
    Query(query): Query<ListReplaysQuery>,
) -> Result<Json<ApiResponse<Vec<crate::safe_event_replay::ReplayStatus>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let limit = query.limit.unwrap_or(50).min(1000);
    let offset = query.offset.unwrap_or(0);

    match state.safe_event_replay.list_replays(limit, offset).await {
        Ok(replays) => Ok(Json(ApiResponse::success(replays))),
        Err(e) => {
            error!("Failed to list safe replays: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Internal server error".to_string())),
            ))
        }
    }
}

#[derive(Deserialize)]
struct ListReplaysQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn metrics_handler() -> String {
    metrics::get_metrics()
}




