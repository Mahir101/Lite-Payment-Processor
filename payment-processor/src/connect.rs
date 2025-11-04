//! # Connect/Marketplace Service
//! 
//! This module handles marketplace functionality, Connect accounts,
//! and split payments to multiple recipients.

use shared::{ConnectAccount, ConnectAccountType, Transfer, TransferStatus, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;

pub struct ConnectService {
    pool: PgPool,
}

impl ConnectService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new Connect account
    pub async fn create_connect_account(
        &self,
        email: Option<String>,
        country: String,
        account_type: ConnectAccountType,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<ConnectAccount, PaymentError> {
        let account_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO connect_accounts (
                id, email, country, type, charges_enabled, payouts_enabled,
                details_submitted, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(account_id)
        .bind(email.as_deref())
        .bind(&country)
        .bind(account_type.to_string())
        .bind(false) // Initially disabled
        .bind(false)
        .bind(false)
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create connect account: {}", e)))?;

        Ok(ConnectAccount {
            id: account_id,
            email,
            country,
            r#type: account_type,
            charges_enabled: false,
            payouts_enabled: false,
            details_submitted: false,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Gets a Connect account by ID
    pub async fn get_connect_account(&self, account_id: Uuid) -> Result<Option<ConnectAccount>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, country, type, charges_enabled, payouts_enabled,
                   details_submitted, metadata, created_at, updated_at
            FROM connect_accounts
            WHERE id = $1
            "#
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get connect account: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_connect_account(row)?)),
            None => Ok(None),
        }
    }

    /// Creates a transfer (split payment) to a Connect account
    pub async fn create_transfer(
        &self,
        transaction_id: Uuid,
        destination_account_id: Uuid,
        amount: i64,
        currency: String,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Transfer, PaymentError> {
        // Verify destination account exists and is enabled
        let account = self.get_connect_account(destination_account_id).await?;
        let account = account.ok_or_else(|| PaymentError::AccountNotFound("Connect account not found".to_string()))?;

        if !account.payouts_enabled {
            return Err(PaymentError::DatabaseError("Destination account does not have payouts enabled".to_string()));
        }

        let transfer_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO transfers (
                id, transaction_id, destination_account_id, amount, currency,
                status, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(transfer_id)
        .bind(transaction_id)
        .bind(destination_account_id)
        .bind(amount)
        .bind(&currency)
        .bind(TransferStatus::Pending.to_string())
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create transfer: {}", e)))?;

        // Process transfer (simulate)
        let final_status = TransferStatus::Paid;
        
        sqlx::query("UPDATE transfers SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(final_status.to_string())
            .bind(Utc::now())
            .bind(transfer_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to update transfer: {}", e)))?;

        Ok(Transfer {
            id: transfer_id,
            transaction_id,
            destination_account_id,
            amount,
            currency,
            status: final_status,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: Utc::now(),
        })
    }

    /// Lists transfers for a transaction
    pub async fn list_transfers_for_transaction(
        &self,
        transaction_id: Uuid,
    ) -> Result<Vec<Transfer>, PaymentError> {
        let rows = sqlx::query(
            r#"
            SELECT id, transaction_id, destination_account_id, amount, currency,
                   status, metadata, created_at, updated_at
            FROM transfers
            WHERE transaction_id = $1
            ORDER BY created_at DESC
            "#
        )
        .bind(transaction_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list transfers: {}", e)))?;

        let mut transfers = Vec::new();
        for row in rows {
            transfers.push(Self::row_to_transfer(row)?);
        }

        Ok(transfers)
    }

    /// Updates Connect account status
    pub async fn update_account_status(
        &self,
        account_id: Uuid,
        charges_enabled: Option<bool>,
        payouts_enabled: Option<bool>,
        details_submitted: Option<bool>,
    ) -> Result<ConnectAccount, PaymentError> {
        let account = self.get_connect_account(account_id).await?
            .ok_or_else(|| PaymentError::AccountNotFound("Connect account not found".to_string()))?;

        let new_charges_enabled = charges_enabled.unwrap_or(account.charges_enabled);
        let new_payouts_enabled = payouts_enabled.unwrap_or(account.payouts_enabled);
        let new_details_submitted = details_submitted.unwrap_or(account.details_submitted);

        sqlx::query(
            r#"
            UPDATE connect_accounts
            SET charges_enabled = $1, payouts_enabled = $2, details_submitted = $3, updated_at = $4
            WHERE id = $5
            "#
        )
        .bind(new_charges_enabled)
        .bind(new_payouts_enabled)
        .bind(new_details_submitted)
        .bind(Utc::now())
        .bind(account_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to update account: {}", e)))?;

        Ok(ConnectAccount {
            charges_enabled: new_charges_enabled,
            payouts_enabled: new_payouts_enabled,
            details_submitted: new_details_submitted,
            ..account
        })
    }

    fn row_to_connect_account(row: sqlx::postgres::PgRow) -> Result<ConnectAccount, PaymentError> {
        let type_str: String = row.get("type");
        let account_type = match type_str.as_str() {
            "express" => ConnectAccountType::Express,
            "standard" => ConnectAccountType::Standard,
            "custom" => ConnectAccountType::Custom,
            _ => return Err(PaymentError::DatabaseError("Invalid connect account type".to_string())),
        };

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(ConnectAccount {
            id: row.get("id"),
            email: row.get("email"),
            country: row.get("country"),
            r#type: account_type,
            charges_enabled: row.get("charges_enabled"),
            payouts_enabled: row.get("payouts_enabled"),
            details_submitted: row.get("details_submitted"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    fn row_to_transfer(row: sqlx::postgres::PgRow) -> Result<Transfer, PaymentError> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "PENDING" => TransferStatus::Pending,
            "PAID" => TransferStatus::Paid,
            "FAILED" => TransferStatus::Failed,
            "CANCELED" => TransferStatus::Canceled,
            _ => return Err(PaymentError::DatabaseError("Invalid transfer status".to_string())),
        };

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(Transfer {
            id: row.get("id"),
            transaction_id: row.get("transaction_id"),
            destination_account_id: row.get("destination_account_id"),
            amount: row.get("amount"),
            currency: row.get("currency"),
            status,
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

impl std::fmt::Display for ConnectAccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectAccountType::Express => write!(f, "express"),
            ConnectAccountType::Standard => write!(f, "standard"),
            ConnectAccountType::Custom => write!(f, "custom"),
        }
    }
}

impl std::fmt::Display for TransferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferStatus::Pending => write!(f, "PENDING"),
            TransferStatus::Paid => write!(f, "PAID"),
            TransferStatus::Failed => write!(f, "FAILED"),
            TransferStatus::Canceled => write!(f, "CANCELED"),
        }
    }
}

