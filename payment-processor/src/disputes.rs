//! # Dispute Service
//! 
//! This module handles dispute and chargeback management, including
//! evidence submission and dispute lifecycle management.

use shared::{Dispute, DisputeStatus, DisputeReason, DisputeEvidence, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{Utc, Duration};

pub struct DisputeService {
    pool: PgPool,
}

impl DisputeService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new dispute for a transaction
    pub async fn create_dispute(
        &self,
        transaction_id: Uuid,
        reason: Option<DisputeReason>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Dispute, PaymentError> {
        // Get transaction to verify it exists
        let transaction = sqlx::query("SELECT id, amount, currency FROM transactions WHERE id = $1")
            .bind(transaction_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to get transaction: {}", e)))?;

        let (amount, currency) = match transaction {
            Some(row) => (row.get::<i64, _>("amount"), row.get::<String, _>("currency")),
            None => return Err(PaymentError::TransactionNotFound(transaction_id)),
        };

        let dispute_id = Uuid::new_v4();
        let now = Utc::now();
        let evidence_due_by = Some(now + Duration::days(7)); // 7 days to respond

        sqlx::query(
            r#"
            INSERT INTO disputes (
                id, transaction_id, amount, currency, status, reason,
                evidence_due_by, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(dispute_id)
        .bind(transaction_id)
        .bind(amount)
        .bind(&currency)
        .bind(DisputeStatus::NeedsResponse.to_string())
        .bind(reason.as_ref().map(|r| r.to_string()))
        .bind(evidence_due_by)
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create dispute: {}", e)))?;

        Ok(Dispute {
            id: dispute_id,
            transaction_id,
            amount,
            currency,
            status: DisputeStatus::NeedsResponse,
            reason,
            evidence_due_by,
            evidence_submitted_at: None,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Gets a dispute by ID
    pub async fn get_dispute(&self, dispute_id: Uuid) -> Result<Option<Dispute>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, transaction_id, amount, currency, status, reason,
                   evidence_due_by, evidence_submitted_at, metadata, created_at, updated_at
            FROM disputes
            WHERE id = $1
            "#
        )
        .bind(dispute_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get dispute: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_dispute(row)?)),
            None => Ok(None),
        }
    }

    /// Lists disputes
    pub async fn list_disputes(
        &self,
        status: Option<DisputeStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Dispute>, PaymentError> {
        let query = if let Some(status) = status {
            sqlx::query(
                r#"
                SELECT id, transaction_id, amount, currency, status, reason,
                       evidence_due_by, evidence_submitted_at, metadata, created_at, updated_at
                FROM disputes
                WHERE status = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#
            )
            .bind(status.to_string())
            .bind(limit)
            .bind(offset)
        } else {
            sqlx::query(
                r#"
                SELECT id, transaction_id, amount, currency, status, reason,
                       evidence_due_by, evidence_submitted_at, metadata, created_at, updated_at
                FROM disputes
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#
            )
            .bind(limit)
            .bind(offset)
        }
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list disputes: {}", e)))?;

        let mut disputes = Vec::new();
        for row in query {
            disputes.push(Self::row_to_dispute(row)?);
        }

        Ok(disputes)
    }

    /// Submits evidence for a dispute
    pub async fn submit_evidence(
        &self,
        dispute_id: Uuid,
        evidence_type: String,
        evidence_data: serde_json::Value,
    ) -> Result<Dispute, PaymentError> {
        // Get dispute
        let dispute = self.get_dispute(dispute_id).await?
            .ok_or_else(|| PaymentError::dispute_error("Dispute not found".to_string()))?;

        // Create evidence record
        let evidence_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO dispute_evidence (id, dispute_id, evidence_type, evidence_data, created_at) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(evidence_id)
        .bind(dispute_id)
        .bind(&evidence_type)
        .bind(&evidence_data)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create evidence: {}", e)))?;

        // Update dispute status
        let new_status = if dispute.status == DisputeStatus::NeedsResponse {
            DisputeStatus::UnderReview
        } else {
            dispute.status
        };

        sqlx::query(
            "UPDATE disputes SET status = $1, evidence_submitted_at = $2, updated_at = $3 WHERE id = $4"
        )
        .bind(new_status.to_string())
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(dispute_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to update dispute: {}", e)))?;

        Ok(Dispute {
            status: new_status,
            evidence_submitted_at: Some(Utc::now()),
            ..dispute
        })
    }

    /// Updates dispute status (e.g., when won/lost)
    pub async fn update_dispute_status(
        &self,
        dispute_id: Uuid,
        status: DisputeStatus,
    ) -> Result<Dispute, PaymentError> {
        sqlx::query("UPDATE disputes SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(status.to_string())
            .bind(Utc::now())
            .bind(dispute_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to update dispute: {}", e)))?;

        self.get_dispute(dispute_id).await?
            .ok_or_else(|| PaymentError::dispute_error("Dispute not found after update".to_string()))
    }

    /// Converts database row to Dispute
    fn row_to_dispute(row: sqlx::postgres::PgRow) -> Result<Dispute, PaymentError> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "WARNING_NEEDS_RESPONSE" => DisputeStatus::WarningNeedsResponse,
            "WARNING_UNDER_REVIEW" => DisputeStatus::WarningUnderReview,
            "WARNING_CLOSED" => DisputeStatus::WarningClosed,
            "NEEDS_RESPONSE" => DisputeStatus::NeedsResponse,
            "UNDER_REVIEW" => DisputeStatus::UnderReview,
            "CHARGE_REFUNDED" => DisputeStatus::ChargeRefunded,
            "WON" => DisputeStatus::Won,
            "LOST" => DisputeStatus::Lost,
            _ => return Err(PaymentError::DatabaseError("Invalid dispute status".to_string())),
        };

        let reason_str: Option<String> = row.get("reason");
        let reason = reason_str.as_deref().and_then(|r| match r {
            "BANK_CANNOT_PROCESS" => Some(DisputeReason::BankCannotProcess),
            "CHECK_RETURNED" => Some(DisputeReason::CheckReturned),
            "CREDIT_NOT_PROCESSED" => Some(DisputeReason::CreditNotProcessed),
            "CUSTOMER_INITIATED" => Some(DisputeReason::CustomerInitiated),
            "DEBIT_NOT_AUTHORIZED" => Some(DisputeReason::DebitNotAuthorized),
            "DUPLICATE" => Some(DisputeReason::Duplicate),
            "FRAUDULENT" => Some(DisputeReason::Fraudulent),
            "GENERAL" => Some(DisputeReason::General),
            "INCORRECT_ACCOUNT_DETAILS" => Some(DisputeReason::IncorrectAccountDetails),
            "INSUFFICIENT_FUNDS" => Some(DisputeReason::InsufficientFunds),
            "PRODUCT_NOT_RECEIVED" => Some(DisputeReason::ProductNotReceived),
            "PRODUCT_UNACCEPTABLE" => Some(DisputeReason::ProductUnacceptable),
            "SUBSCRIPTION_CANCELED" => Some(DisputeReason::SubscriptionCanceled),
            "UNRECOGNIZED" => Some(DisputeReason::Unrecognized),
            _ => None,
        });

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(Dispute {
            id: row.get("id"),
            transaction_id: row.get("transaction_id"),
            amount: row.get("amount"),
            currency: row.get("currency"),
            status,
            reason,
            evidence_due_by: row.get("evidence_due_by"),
            evidence_submitted_at: row.get("evidence_submitted_at"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

impl std::fmt::Display for DisputeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisputeStatus::WarningNeedsResponse => write!(f, "WARNING_NEEDS_RESPONSE"),
            DisputeStatus::WarningUnderReview => write!(f, "WARNING_UNDER_REVIEW"),
            DisputeStatus::WarningClosed => write!(f, "WARNING_CLOSED"),
            DisputeStatus::NeedsResponse => write!(f, "NEEDS_RESPONSE"),
            DisputeStatus::UnderReview => write!(f, "UNDER_REVIEW"),
            DisputeStatus::ChargeRefunded => write!(f, "CHARGE_REFUNDED"),
            DisputeStatus::Won => write!(f, "WON"),
            DisputeStatus::Lost => write!(f, "LOST"),
        }
    }
}

impl std::fmt::Display for DisputeReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisputeReason::BankCannotProcess => write!(f, "BANK_CANNOT_PROCESS"),
            DisputeReason::CheckReturned => write!(f, "CHECK_RETURNED"),
            DisputeReason::CreditNotProcessed => write!(f, "CREDIT_NOT_PROCESSED"),
            DisputeReason::CustomerInitiated => write!(f, "CUSTOMER_INITIATED"),
            DisputeReason::DebitNotAuthorized => write!(f, "DEBIT_NOT_AUTHORIZED"),
            DisputeReason::Duplicate => write!(f, "DUPLICATE"),
            DisputeReason::Fraudulent => write!(f, "FRAUDULENT"),
            DisputeReason::General => write!(f, "GENERAL"),
            DisputeReason::IncorrectAccountDetails => write!(f, "INCORRECT_ACCOUNT_DETAILS"),
            DisputeReason::InsufficientFunds => write!(f, "INSUFFICIENT_FUNDS"),
            DisputeReason::ProductNotReceived => write!(f, "PRODUCT_NOT_RECEIVED"),
            DisputeReason::ProductUnacceptable => write!(f, "PRODUCT_UNACCEPTABLE"),
            DisputeReason::SubscriptionCanceled => write!(f, "SUBSCRIPTION_CANCELED"),
            DisputeReason::Unrecognized => write!(f, "UNRECOGNIZED"),
        }
    }
}

