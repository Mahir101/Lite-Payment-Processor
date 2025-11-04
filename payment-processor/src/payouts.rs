//! # Payout Service
//! 
//! This module handles payout processing, including scheduled and instant payouts.

use shared::{Payout, PayoutStatus, PayoutMethod, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{Utc, Duration};

pub struct PayoutService {
    pool: PgPool,
}

impl PayoutService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new payout
    pub async fn create_payout(
        &self,
        account_id: Uuid,
        amount: i64,
        currency: String,
        payout_method: PayoutMethod,
        arrival_date: Option<chrono::DateTime<Utc>>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Payout, PaymentError> {
        // Verify account exists
        let account = sqlx::query("SELECT id FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to verify account: {}", e)))?;

        if account.is_none() {
            return Err(PaymentError::AccountNotFound("Account not found".to_string()));
        }

        let payout_id = Uuid::new_v4();
        let now = Utc::now();
        
        // Calculate arrival date based on payout method
        let final_arrival_date = arrival_date.or_else(|| {
            match payout_method {
                PayoutMethod::Instant => Some(now + Duration::hours(1)),
                PayoutMethod::BankAccount => Some(now + Duration::days(2)),
                PayoutMethod::Card => Some(now + Duration::days(1)),
            }
        });

        sqlx::query(
            r#"
            INSERT INTO payouts (
                id, account_id, amount, currency, status, payout_method,
                arrival_date, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(payout_id)
        .bind(account_id)
        .bind(amount)
        .bind(&currency)
        .bind(PayoutStatus::Pending.to_string())
        .bind(payout_method.to_string())
        .bind(final_arrival_date)
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create payout: {}", e)))?;

        // Process payout (simulate processing)
        // In production, this would call bank/payment processor API
        let final_status = PayoutStatus::InTransit;
        
        sqlx::query("UPDATE payouts SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(final_status.to_string())
            .bind(Utc::now())
            .bind(payout_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to update payout: {}", e)))?;

        Ok(Payout {
            id: payout_id,
            account_id,
            amount,
            currency,
            status: final_status,
            payout_method,
            arrival_date: final_arrival_date,
            failure_code: None,
            failure_message: None,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: Utc::now(),
        })
    }

    /// Gets a payout by ID
    pub async fn get_payout(&self, payout_id: Uuid) -> Result<Option<Payout>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, account_id, amount, currency, status, payout_method,
                   arrival_date, failure_code, failure_message, metadata, created_at, updated_at
            FROM payouts
            WHERE id = $1
            "#
        )
        .bind(payout_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get payout: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_payout(row)?)),
            None => Ok(None),
        }
    }

    /// Lists payouts
    pub async fn list_payouts(
        &self,
        account_id: Option<Uuid>,
        status: Option<PayoutStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Payout>, PaymentError> {
        let query = match (account_id, status) {
            (Some(aid), Some(st)) => {
                sqlx::query(
                    r#"
                    SELECT id, account_id, amount, currency, status, payout_method,
                           arrival_date, failure_code, failure_message, metadata, created_at, updated_at
                    FROM payouts
                    WHERE account_id = $1 AND status = $2
                    ORDER BY created_at DESC
                    LIMIT $3 OFFSET $4
                    "#
                )
                .bind(aid)
                .bind(st.to_string())
                .bind(limit)
                .bind(offset)
            }
            (Some(aid), None) => {
                sqlx::query(
                    r#"
                    SELECT id, account_id, amount, currency, status, payout_method,
                           arrival_date, failure_code, failure_message, metadata, created_at, updated_at
                    FROM payouts
                    WHERE account_id = $1
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#
                )
                .bind(aid)
                .bind(limit)
                .bind(offset)
            }
            (None, Some(st)) => {
                sqlx::query(
                    r#"
                    SELECT id, account_id, amount, currency, status, payout_method,
                           arrival_date, failure_code, failure_message, metadata, created_at, updated_at
                    FROM payouts
                    WHERE status = $1
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#
                )
                .bind(st.to_string())
                .bind(limit)
                .bind(offset)
            }
            (None, None) => {
                sqlx::query(
                    r#"
                    SELECT id, account_id, amount, currency, status, payout_method,
                           arrival_date, failure_code, failure_message, metadata, created_at, updated_at
                    FROM payouts
                    ORDER BY created_at DESC
                    LIMIT $1 OFFSET $2
                    "#
                )
                .bind(limit)
                .bind(offset)
            }
        }
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list payouts: {}", e)))?;

        let mut payouts = Vec::new();
        for row in query {
            payouts.push(Self::row_to_payout(row)?);
        }

        Ok(payouts)
    }

    /// Cancels a payout (if still pending)
    pub async fn cancel_payout(&self, payout_id: Uuid) -> Result<Payout, PaymentError> {
        let payout = self.get_payout(payout_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Payout not found".to_string()))?;

        if payout.status != PayoutStatus::Pending {
            return Err(PaymentError::DatabaseError("Only pending payouts can be canceled".to_string()));
        }

        sqlx::query("UPDATE payouts SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(PayoutStatus::Canceled.to_string())
            .bind(Utc::now())
            .bind(payout_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to cancel payout: {}", e)))?;

        self.get_payout(payout_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Payout not found after cancelation".to_string()))
    }

    /// Converts database row to Payout
    fn row_to_payout(row: sqlx::postgres::PgRow) -> Result<Payout, PaymentError> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "PENDING" => PayoutStatus::Pending,
            "IN_TRANSIT" => PayoutStatus::InTransit,
            "PAID" => PayoutStatus::Paid,
            "FAILED" => PayoutStatus::Failed,
            "CANCELED" => PayoutStatus::Canceled,
            _ => return Err(PaymentError::DatabaseError("Invalid payout status".to_string())),
        };

        let method_str: String = row.get("payout_method");
        let payout_method = match method_str.as_str() {
            "bank_account" => PayoutMethod::BankAccount,
            "card" => PayoutMethod::Card,
            "instant" => PayoutMethod::Instant,
            _ => return Err(PaymentError::DatabaseError("Invalid payout method".to_string())),
        };

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(Payout {
            id: row.get("id"),
            account_id: row.get("account_id"),
            amount: row.get("amount"),
            currency: row.get("currency"),
            status,
            payout_method,
            arrival_date: row.get("arrival_date"),
            failure_code: row.get("failure_code"),
            failure_message: row.get("failure_message"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

impl std::fmt::Display for PayoutStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayoutStatus::Pending => write!(f, "PENDING"),
            PayoutStatus::InTransit => write!(f, "IN_TRANSIT"),
            PayoutStatus::Paid => write!(f, "PAID"),
            PayoutStatus::Failed => write!(f, "FAILED"),
            PayoutStatus::Canceled => write!(f, "CANCELED"),
        }
    }
}

impl std::fmt::Display for PayoutMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayoutMethod::BankAccount => write!(f, "bank_account"),
            PayoutMethod::Card => write!(f, "card"),
            PayoutMethod::Instant => write!(f, "instant"),
        }
    }
}

