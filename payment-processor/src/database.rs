use anyhow::Result;
use chrono::Utc;
use shared::{
    PaymentError, PaymentRequest, Transaction, TransactionEvent, TransactionEventType,
    TransactionState,
};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct DatabaseService {
    pub pool: PgPool,
}

impl DatabaseService {
    pub async fn new() -> Result<Self, PaymentError> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/payment_processor".to_string());

        let pool = PgPool::connect(&database_url)
            .await
            .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        Ok(Self { pool })
    }

    pub async fn health_check(&self) -> Result<(), PaymentError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub async fn create_transaction(&self, request: PaymentRequest) -> Result<Transaction, PaymentError> {
        let mut tx = self.pool.begin().await
            .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        let transaction = Transaction {
            id,
            external_id: request.external_id,
            amount: request.amount,
            currency: request.currency,
            from_account: request.from_account,
            to_account: request.to_account,
            description: request.description,
            state: TransactionState::Pending,
            created_at: now,
            updated_at: now,
            metadata: request.metadata.unwrap_or_default(),
        };

        // Insert transaction
        sqlx::query(
            r#"
            INSERT INTO transactions (
                id, external_id, amount, currency, from_account, to_account,
                description, state, created_at, updated_at, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(&transaction.id)
        .bind(&transaction.external_id)
        .bind(transaction.amount)
        .bind(&transaction.currency)
        .bind(&transaction.from_account)
        .bind(&transaction.to_account)
        .bind(&transaction.description)
        .bind(&transaction.state.to_string())
        .bind(&transaction.created_at)
        .bind(&transaction.updated_at)
        .bind(&serde_json::to_value(&transaction.metadata).unwrap())
        .execute(&mut *tx)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        // Add to outbox for transactional event publishing
        let outbox_service = crate::outbox::OutboxService::new(self.pool.clone());
        outbox_service.add_event(
            &mut tx,
            transaction.id,
            "Transaction",
            "Created",
            serde_json::to_value(&transaction).unwrap(),
        ).await?;

        tx.commit().await
            .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        Ok(transaction)
    }

    pub async fn get_transaction(&self, id: Uuid) -> Result<Option<Transaction>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, external_id, amount, currency, from_account, to_account,
                   description, state, created_at, updated_at, metadata
            FROM transactions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_transaction(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn update_transaction_state(
        &self,
        id: Uuid,
        new_state: TransactionState,
    ) -> Result<Transaction, PaymentError> {
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE transactions
            SET state = $1, updated_at = $2
            WHERE id = $3
            "#,
        )
        .bind(&new_state.to_string())
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        self.get_transaction(id)
            .await?
            .ok_or_else(|| PaymentError::TransactionNotFound(id))
    }

    pub async fn list_transactions(
        &self,
        state_filter: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>, PaymentError> {
        let query = if let Some(state) = state_filter {
            sqlx::query(
                r#"
                SELECT id, external_id, amount, currency, from_account, to_account,
                       description, state, created_at, updated_at, metadata
                FROM transactions
                WHERE state = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(state)
            .bind(limit)
            .bind(offset)
        } else {
            sqlx::query(
                r#"
                SELECT id, external_id, amount, currency, from_account, to_account,
                       description, state, created_at, updated_at, metadata
                FROM transactions
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
        };

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(self.row_to_transaction(row)?);
        }

        Ok(transactions)
    }

    pub async fn emit_event(
        &self,
        transaction: &Transaction,
        event_type: TransactionEventType,
    ) -> Result<(), PaymentError> {
        let event = TransactionEvent {
            event_id: Uuid::new_v4(),
            transaction_id: transaction.id,
            event_type,
            timestamp: Utc::now(),
            data: serde_json::to_value(transaction).unwrap(),
        };

        sqlx::query(
            r#"
            INSERT INTO transaction_events (id, transaction_id, event_type, event_data, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&event.event_id)
        .bind(&event.transaction_id)
        .bind(&serde_json::to_string(&event.event_type).unwrap())
        .bind(&event.data)
        .bind(&event.timestamp)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn row_to_transaction(&self, row: sqlx::postgres::PgRow) -> Result<Transaction, PaymentError> {
        let state_str: String = row.get("state");
        let state = match state_str.as_str() {
            "PENDING" => TransactionState::Pending,
            "COMMITTED" => TransactionState::Committed,
            "FAILED" => TransactionState::Failed,
            "CANCELLED" => TransactionState::Cancelled,
            _ => return Err(PaymentError::InvalidFormat(format!("Invalid state: {}", state_str))),
        };

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(Transaction {
            id: row.get("id"),
            external_id: row.get("external_id"),
            amount: row.get("amount"),
            currency: row.get("currency"),
            from_account: row.get("from_account"),
            to_account: row.get("to_account"),
            description: row.get("description"),
            state,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            metadata,
        })
    }
}



