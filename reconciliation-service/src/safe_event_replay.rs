use anyhow::Result;
use chrono::{DateTime, Utc};
use shared::{
    TransactionEvent,
    TransactionEventType,
};
use sqlx::{PgPool, Row};
use tracing::{info, error};
use uuid::Uuid;

#[derive(Clone)]
pub struct SafeEventReplayService {
    production_pool: PgPool,
    staging_pool: PgPool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplayStatus {
    pub replay_id: Uuid,
    pub status: ReplayState,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub events_processed: i64,
    pub events_total: i64,
    pub errors_count: i64,
    pub error_message: Option<String>,
    pub backup_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ReplayState {
    Running,
    Completed,
    Failed,
    Cancelled,
    Validating,
    Swapping,
}

impl std::fmt::Display for ReplayState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayState::Running => write!(f, "RUNNING"),
            ReplayState::Completed => write!(f, "COMPLETED"),
            ReplayState::Failed => write!(f, "FAILED"),
            ReplayState::Cancelled => write!(f, "CANCELLED"),
            ReplayState::Validating => write!(f, "VALIDATING"),
            ReplayState::Swapping => write!(f, "SWAPPING"),
        }
    }
}

impl SafeEventReplayService {
    pub fn new(production_pool: PgPool, staging_pool: PgPool) -> Self {
        Self {
            production_pool,
            staging_pool,
        }
    }

    /// Start a safe event replay using staging database approach
    pub async fn start_safe_replay(&self) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let replay_id = Uuid::new_v4();
        let now = Utc::now();

        // Create replay record
        sqlx::query(
            r#"
            INSERT INTO event_replays (replay_id, status, started_at, events_processed, events_total, errors_count)
            VALUES ($1, 'RUNNING', $2, 0, 0, 0)
            "#,
        )
        .bind(&replay_id)
        .bind(&now)
        .execute(&self.production_pool)
        .await?;

        info!("Started safe event replay: {}", replay_id);

        // Start replay in background
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(e) = service.execute_safe_replay(replay_id).await {
                error!("Safe event replay {} failed: {}", replay_id, e);
                service.mark_replay_failed(replay_id, e.to_string()).await.ok();
            }
        });

        Ok(replay_id)
    }

    /// Execute the safe replay process
    async fn execute_safe_replay(&self, replay_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Step 1: Create backup of current data
        let backup_id = self.create_backup().await?;
        self.update_replay_backup_id(replay_id, &backup_id).await?;
        info!("Created backup: {} for replay: {}", backup_id, replay_id);

        // Step 2: Create staging schema
        self.create_staging_schema().await?;
        info!("Created staging schema for replay: {}", replay_id);

        // Step 3: Get all events from payment processor
        let events = self.get_all_events_from_source().await?;
        self.update_replay_progress(replay_id, 0, events.len() as i64).await?;

        // Step 4: Replay events to staging database
        let mut processed = 0;
        let mut errors = 0;

        for event in &events {
            match self.process_replay_event_to_staging(&event).await {
                Ok(_) => {
                    processed += 1;
                    if processed % 100 == 0 {
                        self.update_replay_progress(replay_id, processed, events.len() as i64).await?;
                        info!("Replay {}: processed {}/{} events", replay_id, processed, events.len());
                    }
                }
                Err(e) => {
                    errors += 1;
                    error!("Failed to process event {} in replay {}: {}", event.event_id, replay_id, e);
                    self.update_replay_errors(replay_id, errors).await?;
                }
            }
        }

        // Step 5: Validate staging data
        self.update_replay_status(replay_id, ReplayState::Validating).await?;
        self.validate_staging_data().await?;
        info!("Validation completed for replay: {}", replay_id);

        // Step 6: Atomic swap tables
        self.update_replay_status(replay_id, ReplayState::Swapping).await?;
        self.atomic_swap_tables().await?;
        info!("Atomic swap completed for replay: {}", replay_id);

        // Step 7: Mark as completed
        self.mark_replay_completed(replay_id, processed, errors).await?;
        info!("Safe event replay {} completed: {} events processed, {} errors", replay_id, processed, errors);

        Ok(())
    }

    /// Create backup of current production data
    async fn create_backup(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let backup_id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_suffix = format!("{}_{}", backup_id, timestamp);

        // Create backup tables
        sqlx::query(&format!(
            "CREATE TABLE event_ledger_backup_{} AS SELECT * FROM event_ledger",
            backup_suffix
        ))
        .execute(&self.production_pool)
        .await?;

        sqlx::query(&format!(
            "CREATE TABLE daily_summaries_backup_{} AS SELECT * FROM daily_summaries",
            backup_suffix
        ))
        .execute(&self.production_pool)
        .await?;

        sqlx::query(&format!(
            "CREATE TABLE anomalies_backup_{} AS SELECT * FROM anomalies",
            backup_suffix
        ))
        .execute(&self.production_pool)
        .await?;

        Ok(backup_suffix)
    }

    /// Create staging schema in staging database
    async fn create_staging_schema(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create staging event ledger table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS staging_event_ledger (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                event_id VARCHAR(255) NOT NULL UNIQUE,
                event_type VARCHAR(50) NOT NULL,
                transaction_id UUID NOT NULL,
                event_data JSONB NOT NULL DEFAULT '{}'::jsonb,
                processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.staging_pool)
        .await?;

        // Create staging daily summaries table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS staging_daily_summaries (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                date DATE NOT NULL UNIQUE,
                total_transactions INTEGER NOT NULL DEFAULT 0,
                total_amount BIGINT NOT NULL DEFAULT 0,
                committed_transactions INTEGER NOT NULL DEFAULT 0,
                committed_amount BIGINT NOT NULL DEFAULT 0,
                failed_transactions INTEGER NOT NULL DEFAULT 0,
                failed_amount BIGINT NOT NULL DEFAULT 0,
                anomalies_count INTEGER NOT NULL DEFAULT 0,
                last_reconciliation_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.staging_pool)
        .await?;

        // Create staging anomalies table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS staging_anomalies (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                anomaly_type VARCHAR(50) NOT NULL,
                description TEXT NOT NULL,
                transaction_id UUID,
                expected_value JSONB,
                actual_value JSONB,
                severity VARCHAR(20) NOT NULL CHECK (severity IN ('LOW', 'MEDIUM', 'HIGH', 'CRITICAL')),
                status VARCHAR(20) NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN', 'INVESTIGATING', 'RESOLVED', 'IGNORED')),
                detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                resolved_at TIMESTAMPTZ,
                resolved_by VARCHAR(255),
                resolution_notes TEXT
            )
            "#,
        )
        .execute(&self.staging_pool)
        .await?;

        // Clear any existing staging data
        sqlx::query("DELETE FROM staging_event_ledger").execute(&self.staging_pool).await?;
        sqlx::query("DELETE FROM staging_daily_summaries").execute(&self.staging_pool).await?;
        sqlx::query("DELETE FROM staging_anomalies").execute(&self.staging_pool).await?;

        Ok(())
    }

    /// Get all events from the payment processor
    async fn get_all_events_from_source(&self) -> Result<Vec<TransactionEvent>, Box<dyn std::error::Error + Send + Sync>> {
        // This queries the payment processor database for events
        // In a real scenario, you would connect to the payment processor database
        let rows = sqlx::query(
            r#"
            SELECT event_id, transaction_id, event_type, event_data, created_at
            FROM transaction_events
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.production_pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            events.push(TransactionEvent {
                event_id: row.get("event_id"),
                transaction_id: row.get("transaction_id"),
                event_type: self.parse_event_type(row.get("event_type")),
                timestamp: row.get("created_at"),
                data: row.get("event_data"),
            });
        }

        Ok(events)
    }

    /// Process a single event during replay to staging database
    async fn process_replay_event_to_staging(&self, event: &TransactionEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Store event in staging ledger
        self.store_event_to_staging(event).await?;
        
        // Update staging daily summary
        self.update_daily_summary_from_event_to_staging(event).await?;

        Ok(())
    }

    /// Store event in the staging reconciliation ledger
    async fn store_event_to_staging(&self, event: &TransactionEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            INSERT INTO staging_event_ledger (event_id, transaction_id, event_type, event_data, processed_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (event_id) DO NOTHING
            "#,
        )
        .bind(&event.event_id)
        .bind(&event.transaction_id)
        .bind(&serde_json::to_string(&event.event_type)?)
        .bind(&event.data)
        .bind(&event.timestamp)
        .bind(&event.timestamp)
        .execute(&self.staging_pool)
        .await?;

        Ok(())
    }

    /// Update staging daily summary from event
    async fn update_daily_summary_from_event_to_staging(&self, event: &TransactionEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event_date = event.timestamp.date_naive();
        
        // Extract amount from event data if available
        let amount = self.extract_amount_from_event(event);
        
        sqlx::query(
            r#"
            INSERT INTO staging_daily_summaries (date, total_transactions, total_amount, committed_count, failed_count, pending_count)
            VALUES ($1, 1, $2, 0, 0, 1)
            ON CONFLICT (date) DO UPDATE SET
                total_transactions = staging_daily_summaries.total_transactions + 1,
                total_amount = staging_daily_summaries.total_amount + $2,
                pending_count = staging_daily_summaries.pending_count + 1,
                updated_at = NOW()
            "#,
        )
        .bind(event_date)
        .bind(amount)
        .execute(&self.staging_pool)
        .await?;

        Ok(())
    }

    /// Validate staging data before swapping
    async fn validate_staging_data(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check row counts match
        let original_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_ledger")
            .fetch_one(&self.production_pool)
            .await?;
        
        let staging_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM staging_event_ledger")
            .fetch_one(&self.staging_pool)
            .await?;

        if original_count != staging_count {
            return Err(anyhow::anyhow!("Event count mismatch: original={}, staging={}", original_count, staging_count).into());
        }

        // Check data integrity
        self.validate_data_integrity().await?;

        info!("Staging data validation passed: {} events", staging_count);
        Ok(())
    }

    /// Validate data integrity between production and staging
    async fn validate_data_integrity(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check for duplicate event_ids in staging
        let duplicate_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (
                SELECT event_id, COUNT(*) as cnt 
                FROM staging_event_ledger 
                GROUP BY event_id 
                HAVING COUNT(*) > 1
            ) as duplicates"
        )
        .fetch_one(&self.staging_pool)
        .await?;

        if duplicate_count > 0 {
            return Err(anyhow::anyhow!("Found {} duplicate events in staging data", duplicate_count).into());
        }

        // Check for null required fields
        let null_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM staging_event_ledger WHERE event_id IS NULL OR transaction_id IS NULL"
        )
        .fetch_one(&self.staging_pool)
        .await?;

        if null_count > 0 {
            return Err(anyhow::anyhow!("Found {} events with null required fields", null_count).into());
        }

        Ok(())
    }

    /// Perform atomic swap of tables
    async fn atomic_swap_tables(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut tx = self.production_pool.begin().await?;

        // Rename current tables to backup
        sqlx::query("ALTER TABLE event_ledger RENAME TO event_ledger_old")
            .execute(&mut *tx).await?;
        sqlx::query("ALTER TABLE daily_summaries RENAME TO daily_summaries_old")
            .execute(&mut *tx).await?;
        sqlx::query("ALTER TABLE anomalies RENAME TO anomalies_old")
            .execute(&mut *tx).await?;

        // Copy staging tables to production
        sqlx::query("CREATE TABLE event_ledger AS SELECT * FROM staging_event_ledger")
            .execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE daily_summaries AS SELECT * FROM staging_daily_summaries")
            .execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE anomalies AS SELECT * FROM staging_anomalies")
            .execute(&mut *tx).await?;

        // Recreate indexes
        sqlx::query("CREATE INDEX idx_event_ledger_event_id ON event_ledger(event_id)")
            .execute(&mut *tx).await?;
        sqlx::query("CREATE INDEX idx_event_ledger_transaction_id ON event_ledger(transaction_id)")
            .execute(&mut *tx).await?;
        sqlx::query("CREATE INDEX idx_daily_summaries_date ON daily_summaries(date)")
            .execute(&mut *tx).await?;

        tx.commit().await?;

        // Clean up staging tables
        sqlx::query("DROP TABLE staging_event_ledger").execute(&self.staging_pool).await?;
        sqlx::query("DROP TABLE staging_daily_summaries").execute(&self.staging_pool).await?;
        sqlx::query("DROP TABLE staging_anomalies").execute(&self.staging_pool).await?;

        info!("Atomic table swap completed successfully");
        Ok(())
    }

    /// Extract amount from event data
    fn extract_amount_from_event(&self, event: &TransactionEvent) -> i64 {
        if let Some(amount) = event.data.get("amount").and_then(|v| v.as_i64()) {
            amount
        } else {
            0
        }
    }

    /// Update replay progress
    async fn update_replay_progress(&self, replay_id: Uuid, processed: i64, total: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            UPDATE event_replays
            SET events_processed = $1, events_total = $2
            WHERE replay_id = $3
            "#,
        )
        .bind(processed)
        .bind(total)
        .bind(&replay_id)
        .execute(&self.production_pool)
        .await?;

        Ok(())
    }

    /// Update error count
    async fn update_replay_errors(&self, replay_id: Uuid, errors: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            UPDATE event_replays
            SET errors_count = $1
            WHERE replay_id = $2
            "#,
        )
        .bind(errors)
        .bind(&replay_id)
        .execute(&self.production_pool)
        .await?;

        Ok(())
    }

    /// Update replay status
    async fn update_replay_status(&self, replay_id: Uuid, status: ReplayState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            UPDATE event_replays
            SET status = $1
            WHERE replay_id = $2
            "#,
        )
        .bind(status.to_string())
        .bind(&replay_id)
        .execute(&self.production_pool)
        .await?;

        Ok(())
    }

    /// Update replay backup ID
    async fn update_replay_backup_id(&self, replay_id: Uuid, backup_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            UPDATE event_replays
            SET error_message = $1
            WHERE replay_id = $2
            "#,
        )
        .bind(format!("BACKUP:{}", backup_id))
        .bind(&replay_id)
        .execute(&self.production_pool)
        .await?;

        Ok(())
    }

    /// Mark replay as completed
    async fn mark_replay_completed(&self, replay_id: Uuid, processed: i64, errors: i64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            UPDATE event_replays
            SET status = 'COMPLETED', completed_at = NOW(), events_processed = $1, errors_count = $2
            WHERE replay_id = $3
            "#,
        )
        .bind(processed)
        .bind(errors)
        .bind(&replay_id)
        .execute(&self.production_pool)
        .await?;

        Ok(())
    }

    /// Mark replay as failed
    async fn mark_replay_failed(&self, replay_id: Uuid, error_message: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            UPDATE event_replays
            SET status = 'FAILED', completed_at = NOW(), error_message = $1
            WHERE replay_id = $2
            "#,
        )
        .bind(&error_message)
        .bind(&replay_id)
        .execute(&self.production_pool)
        .await?;

        Ok(())
    }

    /// Get replay status
    pub async fn get_replay_status(&self, replay_id: Uuid) -> Result<Option<ReplayStatus>, Box<dyn std::error::Error + Send + Sync>> {
        let row = sqlx::query(
            r#"
            SELECT replay_id, status, started_at, completed_at, events_processed, events_total, errors_count, error_message
            FROM event_replays
            WHERE replay_id = $1
            "#,
        )
        .bind(&replay_id)
        .fetch_optional(&self.production_pool)
        .await?;

        if let Some(row) = row {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "RUNNING" => ReplayState::Running,
                "COMPLETED" => ReplayState::Completed,
                "FAILED" => ReplayState::Failed,
                "CANCELLED" => ReplayState::Cancelled,
                "VALIDATING" => ReplayState::Validating,
                "SWAPPING" => ReplayState::Swapping,
                _ => ReplayState::Failed,
            };

            let error_message: Option<String> = row.get("error_message");
            let backup_id = error_message
                .as_ref()
                .and_then(|msg| msg.strip_prefix("BACKUP:"))
                .map(|s| s.to_string());

            Ok(Some(ReplayStatus {
                replay_id: row.get("replay_id"),
                status,
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                events_processed: row.get("events_processed"),
                events_total: row.get("events_total"),
                errors_count: row.get("errors_count"),
                error_message: error_message,
                backup_id,
            }))
        } else {
            Ok(None)
        }
    }

    /// List all replays
    pub async fn list_replays(&self, limit: i64, offset: i64) -> Result<Vec<ReplayStatus>, Box<dyn std::error::Error + Send + Sync>> {
        let rows = sqlx::query(
            r#"
            SELECT replay_id, status, started_at, completed_at, events_processed, events_total, errors_count, error_message
            FROM event_replays
            ORDER BY started_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.production_pool)
        .await?;

        let mut replays = Vec::new();
        for row in rows {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "RUNNING" => ReplayState::Running,
                "COMPLETED" => ReplayState::Completed,
                "FAILED" => ReplayState::Failed,
                "CANCELLED" => ReplayState::Cancelled,
                "VALIDATING" => ReplayState::Validating,
                "SWAPPING" => ReplayState::Swapping,
                _ => ReplayState::Failed,
            };

            let error_message: Option<String> = row.get("error_message");
            let backup_id = error_message
                .as_ref()
                .and_then(|msg| msg.strip_prefix("BACKUP:"))
                .map(|s| s.to_string());

            replays.push(ReplayStatus {
                replay_id: row.get("replay_id"),
                status,
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                events_processed: row.get("events_processed"),
                events_total: row.get("events_total"),
                errors_count: row.get("errors_count"),
                error_message: error_message,
                backup_id,
            });
        }

        Ok(replays)
    }

    /// Parse event type from string
    fn parse_event_type(&self, event_type_str: String) -> TransactionEventType {
        // This is a simplified parser - in reality you'd have more sophisticated parsing
        if event_type_str.contains("Created") {
            TransactionEventType::Created
        } else if event_type_str.contains("StateChanged") {
            TransactionEventType::StateChanged {
                from: shared::TransactionState::Pending,
                to: shared::TransactionState::Committed,
            }
        } else if event_type_str.contains("Failed") {
            TransactionEventType::Failed {
                reason: "Replay processing".to_string(),
            }
        } else {
            TransactionEventType::Completed
        }
    }
}
