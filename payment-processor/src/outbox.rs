use anyhow::Result;
use chrono::Utc;
use shared::PaymentError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct OutboxService {
    pool: PgPool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutboxEvent {
    pub id: Uuid,
    pub aggregate_id: Uuid,
    pub aggregate_type: String,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub created_at: chrono::DateTime<Utc>,
    pub processed_at: Option<chrono::DateTime<Utc>>,
    pub retry_count: i32,
    pub status: OutboxStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OutboxStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl std::fmt::Display for OutboxStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutboxStatus::Pending => write!(f, "PENDING"),
            OutboxStatus::Processing => write!(f, "PROCESSING"),
            OutboxStatus::Completed => write!(f, "COMPLETED"),
            OutboxStatus::Failed => write!(f, "FAILED"),
        }
    }
}

impl OutboxService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Add an event to the outbox within a transaction
    pub async fn add_event(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        aggregate_id: Uuid,
        aggregate_type: &str,
        event_type: &str,
        event_data: serde_json::Value,
    ) -> Result<Uuid, PaymentError> {
        let event_id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO outbox_events (id, aggregate_id, aggregate_type, event_type, event_data, created_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
        )
        .bind(&event_id)
        .bind(&aggregate_id)
        .bind(aggregate_type)
        .bind(event_type)
        .bind(&event_data)
        .execute(&mut **transaction)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        Ok(event_id)
    }

    /// Get pending events for processing
    pub async fn get_pending_events(&self, limit: i64) -> Result<Vec<OutboxEvent>, PaymentError> {
        let rows = sqlx::query(
            r#"
            SELECT id, aggregate_id, aggregate_type, event_type, event_data, 
                   created_at, processed_at, retry_count, status
            FROM outbox_events
            WHERE status = 'PENDING'
            ORDER BY created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            events.push(self.row_to_outbox_event(row)?);
        }

        Ok(events)
    }

    /// Mark event as processing
    pub async fn mark_as_processing(&self, event_id: Uuid) -> Result<(), PaymentError> {
        sqlx::query(
            r#"
            UPDATE outbox_events
            SET status = 'PROCESSING', processed_at = NOW()
            WHERE id = $1 AND status = 'PENDING'
            "#,
        )
        .bind(&event_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Mark event as completed
    pub async fn mark_as_completed(&self, event_id: Uuid) -> Result<(), PaymentError> {
        sqlx::query(
            r#"
            UPDATE outbox_events
            SET status = 'COMPLETED', processed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(&event_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Mark event as failed and increment retry count
    pub async fn mark_as_failed(&self, event_id: Uuid) -> Result<(), PaymentError> {
        sqlx::query(
            r#"
            UPDATE outbox_events
            SET status = 'FAILED', retry_count = retry_count + 1, processed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(&event_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Reset failed events back to pending for retry (with max retry limit)
    pub async fn reset_failed_events(&self, max_retries: i32) -> Result<(), PaymentError> {
        sqlx::query(
            r#"
            UPDATE outbox_events
            SET status = 'PENDING', processed_at = NULL
            WHERE status = 'FAILED' AND retry_count < $1
            "#,
        )
        .bind(max_retries)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Convert database row to OutboxEvent
    fn row_to_outbox_event(&self, row: sqlx::postgres::PgRow) -> Result<OutboxEvent, PaymentError> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "PENDING" => OutboxStatus::Pending,
            "PROCESSING" => OutboxStatus::Processing,
            "COMPLETED" => OutboxStatus::Completed,
            "FAILED" => OutboxStatus::Failed,
            _ => return Err(PaymentError::InvalidFormat(format!("Invalid outbox status: {}", status_str))),
        };

        Ok(OutboxEvent {
            id: row.get("id"),
            aggregate_id: row.get("aggregate_id"),
            aggregate_type: row.get("aggregate_type"),
            event_type: row.get("event_type"),
            event_data: row.get("event_data"),
            created_at: row.get("created_at"),
            processed_at: row.get("processed_at"),
            retry_count: row.get("retry_count"),
            status,
        })
    }
}

/// Outbox event processor that runs in background
pub struct OutboxProcessor {
    outbox: OutboxService,
    redis: crate::redis_client::RedisService,
}

impl OutboxProcessor {
    pub fn new(outbox: OutboxService, redis: crate::redis_client::RedisService) -> Self {
        Self { outbox, redis }
    }

    /// Process pending outbox events
    pub async fn process_events(&self) -> Result<(), PaymentError> {
        let events = self.outbox.get_pending_events(100).await?;
        
        for event in events {
            if let Err(e) = self.process_single_event(&event).await {
                tracing::error!("Failed to process outbox event {}: {}", event.id, e);
                self.outbox.mark_as_failed(event.id).await?;
            } else {
                self.outbox.mark_as_completed(event.id).await?;
            }
        }

        Ok(())
    }

    async fn process_single_event(&self, event: &OutboxEvent) -> Result<(), PaymentError> {
        // Mark as processing
        self.outbox.mark_as_processing(event.id).await?;

        // Publish to Redis
        let channel = format!("events:{}", event.aggregate_type);
        let event_json = serde_json::to_string(event)
            .map_err(|e| PaymentError::InvalidFormat(e.to_string()))?;
        
        self.redis.publish_event(&channel, &event_json).await?;

        Ok(())
    }

    /// Start the outbox processor as a background task
    pub async fn start_processor(self) -> Result<(), PaymentError> {
        tracing::info!("Starting outbox processor");
        
        loop {
            if let Err(e) = self.process_events().await {
                tracing::error!("Outbox processor error: {}", e);
            }
            
            // Process events every 5 seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }
}

