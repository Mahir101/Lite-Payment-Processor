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
pub struct EventReplayService {
    pool: PgPool,
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ReplayState {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for ReplayState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayState::Running => write!(f, "RUNNING"),
            ReplayState::Completed => write!(f, "COMPLETED"),
            ReplayState::Failed => write!(f, "FAILED"),
            ReplayState::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

impl EventReplayService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Start a complete event replay to rebuild the ledger from scratch
    pub async fn start_full_replay(&self) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
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
        .execute(&self.pool)
        .await?;

        info!("Started event replay: {}", replay_id);

        // Start replay in background
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(e) = service.execute_replay(replay_id).await {
                error!("Event replay {} failed: {}", replay_id, e);
                service.mark_replay_failed(replay_id, e.to_string()).await.ok();
            }
        });

        Ok(replay_id)
    }

    /// Execute the actual replay process
    async fn execute_replay(&self, replay_id: Uuid) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Clear existing ledger data
        self.clear_ledger_data().await?;

        // Get all events from payment processor (this would typically come from a shared database or API)
        let events = self.get_all_events_from_source().await?;
        
        // Update total count
        self.update_replay_progress(replay_id, 0, events.len() as i64).await?;

        let mut processed = 0;
        let mut errors = 0;

        for event in &events {
            match self.process_replay_event(&event).await {
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

        // Mark as completed
        self.mark_replay_completed(replay_id, processed, errors).await?;
        info!("Event replay {} completed: {} events processed, {} errors", replay_id, processed, errors);

        Ok(())
    }

    /// Get all events from the payment processor (simplified - in reality would query payment processor DB)
    async fn get_all_events_from_source(&self) -> Result<Vec<TransactionEvent>, Box<dyn std::error::Error + Send + Sync>> {
        // This is a simplified implementation
        // In a real scenario, you would:
        // 1. Connect to the payment processor database
        // 2. Query all transaction_events
        // 3. Convert them to TransactionEvent structs
        
        let rows = sqlx::query(
            r#"
            SELECT event_id, transaction_id, event_type, event_data, created_at
            FROM transaction_events
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
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

    /// Process a single event during replay
    async fn process_replay_event(&self, event: &TransactionEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Store event in ledger
        self.store_event(event).await?;
        
        // Update daily summary
        self.update_daily_summary_from_event(event).await?;

        Ok(())
    }

    /// Store event in the reconciliation ledger
    async fn store_event(&self, event: &TransactionEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            INSERT INTO event_ledger (event_id, transaction_id, event_type, event_data, processed_at, created_at)
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update daily summary from event
    async fn update_daily_summary_from_event(&self, event: &TransactionEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event_date = event.timestamp.date_naive();
        
        // Extract amount from event data if available
        let amount = self.extract_amount_from_event(event);
        
        sqlx::query(
            r#"
            INSERT INTO daily_summaries (date, total_transactions, total_amount, committed_count, failed_count, pending_count)
            VALUES ($1, 1, $2, 0, 0, 1)
            ON CONFLICT (date) DO UPDATE SET
                total_transactions = daily_summaries.total_transactions + 1,
                total_amount = daily_summaries.total_amount + $2,
                pending_count = daily_summaries.pending_count + 1,
                updated_at = NOW()
            "#,
        )
        .bind(event_date)
        .bind(amount)
        .execute(&self.pool)
        .await?;

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

    /// Clear existing ledger data
    async fn clear_ledger_data(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query("DELETE FROM event_ledger").execute(&self.pool).await?;
        sqlx::query("DELETE FROM daily_summaries").execute(&self.pool).await?;
        sqlx::query("DELETE FROM anomalies").execute(&self.pool).await?;
        Ok(())
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
        .execute(&self.pool)
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
        .execute(&self.pool)
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
        .execute(&self.pool)
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
        .execute(&self.pool)
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
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "RUNNING" => ReplayState::Running,
                "COMPLETED" => ReplayState::Completed,
                "FAILED" => ReplayState::Failed,
                "CANCELLED" => ReplayState::Cancelled,
                _ => ReplayState::Failed,
            };

            Ok(Some(ReplayStatus {
                replay_id: row.get("replay_id"),
                status,
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                events_processed: row.get("events_processed"),
                events_total: row.get("events_total"),
                errors_count: row.get("errors_count"),
                error_message: row.get("error_message"),
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
        .fetch_all(&self.pool)
        .await?;

        let mut replays = Vec::new();
        for row in rows {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "RUNNING" => ReplayState::Running,
                "COMPLETED" => ReplayState::Completed,
                "FAILED" => ReplayState::Failed,
                "CANCELLED" => ReplayState::Cancelled,
                _ => ReplayState::Failed,
            };

            replays.push(ReplayStatus {
                replay_id: row.get("replay_id"),
                status,
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                events_processed: row.get("events_processed"),
                events_total: row.get("events_total"),
                errors_count: row.get("errors_count"),
                error_message: row.get("error_message"),
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
