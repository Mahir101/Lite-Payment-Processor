//! # Refund Service
//! 
//! This module handles refund processing for transactions, including
//! full and partial refunds with proper state management and validation.

use shared::{Refund, RefundStatus, RefundReason, PaymentError, TransactionState};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;

pub struct RefundService {
    pool: PgPool,
}

impl RefundService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a refund for a transaction
    pub async fn create_refund(
        &self,
        transaction_id: Uuid,
        amount: Option<i64>,
        reason: Option<RefundReason>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Refund, PaymentError> {
        // Get transaction to verify it exists and is refundable
        let transaction = sqlx::query(
            "SELECT id, amount, currency, state, refunded_amount FROM transactions WHERE id = $1"
        )
        .bind(transaction_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get transaction: {}", e)))?;

        let tx = match transaction {
            Some(row) => {
                let state_str: String = row.get("state");
                let state = match state_str.as_str() {
                    "COMMITTED" => TransactionState::Committed,
                    _ => return Err(PaymentError::refund_error("Transaction must be COMMITTED to refund".to_string())),
                };
                (
                    row.get::<Uuid, _>("id"),
                    row.get::<i64, _>("amount"),
                    row.get::<String, _>("currency"),
                    row.get::<i64, _>("refunded_amount"),
                    state,
                )
            }
            None => return Err(PaymentError::TransactionNotFound(transaction_id)),
        };

        let (tx_id, tx_amount, currency, refunded_amount, _) = tx;

        // Determine refund amount
        let refund_amount = amount.unwrap_or(tx_amount - refunded_amount);
        
        // Validate refund amount
        if refund_amount <= 0 {
            return Err(PaymentError::refund_error("Refund amount must be greater than 0".to_string()));
        }

        if refunded_amount + refund_amount > tx_amount {
            return Err(PaymentError::refund_error(format!(
                "Total refund amount ({}) exceeds transaction amount ({})",
                refunded_amount + refund_amount,
                tx_amount
            )));
        }

        // Create refund record
        let refund_id = Uuid::new_v4();
        let status = RefundStatus::Pending;
        let reason_str = reason.as_ref().map(|r| r.to_string());

        sqlx::query(
            r#"
            INSERT INTO refunds (id, transaction_id, amount, currency, reason, status, refund_metadata, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(refund_id)
        .bind(transaction_id)
        .bind(refund_amount)
        .bind(&currency)
        .bind(reason_str.as_deref())
        .bind(status.to_string())
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(Utc::now())
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create refund: {}", e)))?;

        // Update transaction refund information
        let new_refunded_amount = refunded_amount + refund_amount;
        sqlx::query(
            "UPDATE transactions SET refunded_amount = $1, refund_count = refund_count + 1 WHERE id = $2"
        )
        .bind(new_refunded_amount)
        .bind(transaction_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to update transaction: {}", e)))?;

        // Process refund (simulate processing)
        // In production, this would call payment processor API
        let final_status = RefundStatus::Succeeded;
        
        sqlx::query(
            "UPDATE refunds SET status = $1, updated_at = $2 WHERE id = $3"
        )
        .bind(final_status.to_string())
        .bind(Utc::now())
        .bind(refund_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to update refund status: {}", e)))?;

        Ok(Refund {
            id: refund_id,
            transaction_id,
            amount: refund_amount,
            currency,
            reason,
            status: final_status,
            metadata: metadata.unwrap_or_default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Gets a refund by ID
    pub async fn get_refund(&self, refund_id: Uuid) -> Result<Option<Refund>, PaymentError> {
        let row = sqlx::query(
            "SELECT id, transaction_id, amount, currency, reason, status, refund_metadata, created_at, updated_at FROM refunds WHERE id = $1"
        )
        .bind(refund_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get refund: {}", e)))?;

        match row {
            Some(row) => {
                let status_str: String = row.get("status");
                let status = match status_str.as_str() {
                    "PENDING" => RefundStatus::Pending,
                    "SUCCEEDED" => RefundStatus::Succeeded,
                    "FAILED" => RefundStatus::Failed,
                    "CANCELLED" => RefundStatus::Cancelled,
                    _ => return Err(PaymentError::DatabaseError("Invalid refund status".to_string())),
                };

                let reason_str: Option<String> = row.get("reason");
                let reason = reason_str.as_deref().and_then(|r| match r {
                    "requested_by_customer" => Some(RefundReason::RequestedByCustomer),
                    "duplicate" => Some(RefundReason::Duplicate),
                    "fraudulent" => Some(RefundReason::Fraudulent),
                    "other" => Some(RefundReason::Other),
                    _ => None,
                });

                let metadata_value: serde_json::Value = row.get("refund_metadata");
                let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
                    .unwrap_or_default();

                Ok(Some(Refund {
                    id: row.get("id"),
                    transaction_id: row.get("transaction_id"),
                    amount: row.get("amount"),
                    currency: row.get("currency"),
                    reason,
                    status,
                    metadata,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Lists refunds for a transaction
    pub async fn list_refunds_for_transaction(
        &self,
        transaction_id: Uuid,
    ) -> Result<Vec<Refund>, PaymentError> {
        let rows = sqlx::query(
            "SELECT id, transaction_id, amount, currency, reason, status, refund_metadata, created_at, updated_at FROM refunds WHERE transaction_id = $1 ORDER BY created_at DESC"
        )
        .bind(transaction_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list refunds: {}", e)))?;

        let mut refunds = Vec::new();
        for row in rows {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "PENDING" => RefundStatus::Pending,
                "SUCCEEDED" => RefundStatus::Succeeded,
                "FAILED" => RefundStatus::Failed,
                "CANCELLED" => RefundStatus::Cancelled,
                _ => continue,
            };

            let reason_str: Option<String> = row.get("reason");
            let reason = reason_str.as_deref().and_then(|r| match r {
                "requested_by_customer" => Some(RefundReason::RequestedByCustomer),
                "duplicate" => Some(RefundReason::Duplicate),
                "fraudulent" => Some(RefundReason::Fraudulent),
                "other" => Some(RefundReason::Other),
                _ => None,
            });

            let metadata_value: serde_json::Value = row.get("refund_metadata");
            let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
                .unwrap_or_default();

            refunds.push(Refund {
                id: row.get("id"),
                transaction_id: row.get("transaction_id"),
                amount: row.get("amount"),
                currency: row.get("currency"),
                reason,
                status,
                metadata,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(refunds)
    }
}

