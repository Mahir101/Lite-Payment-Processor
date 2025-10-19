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
mod metrics;
mod reconciliation;
mod redis_client;

use database::DatabaseService;
use redis_client::RedisService;
use reconciliation::ReconciliationService;
use event_replay::EventReplayService;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseService,
    pub redis: RedisService,
    pub reconciliation: ReconciliationService,
    pub event_replay: EventReplayService,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    info!("Starting Reconciliation & Reporting Service");

    // Initialize metrics
    metrics::init_metrics();

    // Initialize services
    let db = DatabaseService::new().await.map_err(|e| anyhow::anyhow!("{}", e))?;
    let redis = RedisService::new().await.map_err(|e| anyhow::anyhow!("{}", e))?;
    let reconciliation = ReconciliationService::new();
    let event_replay = EventReplayService::new(db.pool.clone());

    let app_state = AppState {
        db,
        redis,
        reconciliation,
        event_replay,
    };

    // Start event consumer
    let app_state_clone = app_state.clone();
    tokio::spawn(async move {
        if let Err(e) = app_state_clone.consume_events().await {
            error!("Event consumer failed: {}", e);
        }
    });

    // Build application
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/reports", get(list_reports))
        .route("/reports/:id", get(get_report))
        .route("/reports/:id/download", get(download_report))
        .route("/reports/generate", post(generate_report))
        .route("/anomalies", get(list_anomalies))
        .route("/anomalies/:id", get(get_anomaly))
        .route("/daily-summaries", get(list_daily_summaries))
        .route("/reconcile", post(trigger_reconciliation))
        .route("/replay/start", post(start_event_replay))
        .route("/replay/:id", get(get_replay_status))
        .route("/replay", get(list_replays))
        .route("/metrics", get(metrics_handler))
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default()))
                .layer(CorsLayer::permissive()),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3002").await?;
    info!("Reconciliation & Reporting Service listening on port 3002");
    
    axum::serve(listener, app).await?;
    Ok(())
}

impl AppState {
    async fn consume_events(&self) -> Result<()> {
        info!("Starting event consumer");
        
        // Simplified event consumption - in a real implementation this would
        // continuously poll Redis for events
        loop {
            match self.redis.subscribe_to_events().await {
                Ok(_) => {
                    // Process events here
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
                Err(e) => {
                    error!("Failed to subscribe to events: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn process_event(&self, event: TransactionEvent) -> Result<()> {
        info!("Processing event: {:?}", event.event_type);
        
        // Store event in ledger
        self.db.store_event(&event).await.map_err(|e| anyhow::anyhow!("{}", e))?;
        
        // Update daily summary
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

#[derive(Deserialize)]
struct ListReplaysQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn metrics_handler() -> String {
    metrics::get_metrics()
}




