//! # Payment Intent Service
//! 
//! This module handles Payment Intents API with 3D Secure support
//! for Strong Customer Authentication (SCA).

use shared::{PaymentIntent, PaymentIntentStatus, ConfirmationMethod, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;

pub struct PaymentIntentService {
    pool: PgPool,
}

impl PaymentIntentService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new Payment Intent
    pub async fn create_payment_intent(
        &self,
        customer_id: Option<Uuid>,
        payment_method_id: Option<Uuid>,
        amount: i64,
        currency: String,
        confirmation_method: ConfirmationMethod,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<PaymentIntent, PaymentError> {
        let intent_id = Uuid::new_v4();
        let client_secret = format!("pi_{}_{}", intent_id, Uuid::new_v4());
        let now = Utc::now();

        let initial_status = if payment_method_id.is_some() {
            PaymentIntentStatus::RequiresConfirmation
        } else {
            PaymentIntentStatus::RequiresPaymentMethod
        };

        sqlx::query(
            r#"
            INSERT INTO payment_intents (
                id, customer_id, payment_method_id, amount, currency, status,
                confirmation_method, client_secret, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#
        )
        .bind(intent_id)
        .bind(customer_id)
        .bind(payment_method_id)
        .bind(amount)
        .bind(&currency)
        .bind(initial_status.to_string())
        .bind(confirmation_method.to_string())
        .bind(&client_secret)
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create payment intent: {}", e)))?;

        Ok(PaymentIntent {
            id: intent_id,
            customer_id,
            payment_method_id,
            amount,
            currency,
            status: initial_status,
            confirmation_method,
            client_secret: Some(client_secret),
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Confirms a Payment Intent (processes the payment)
    pub async fn confirm_payment_intent(
        &self,
        intent_id: Uuid,
        payment_method_id: Option<Uuid>,
    ) -> Result<PaymentIntent, PaymentError> {
        let intent = self.get_payment_intent(intent_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Payment Intent not found".to_string()))?;

        // Check if 3D Secure is required (simplified check)
        let requires_action = Self::requires_3d_secure(&intent);

        let new_status = if requires_action {
            PaymentIntentStatus::RequiresAction
        } else {
            PaymentIntentStatus::Processing
        };

        // Update payment method if provided
        if let Some(pm_id) = payment_method_id {
            sqlx::query("UPDATE payment_intents SET payment_method_id = $1 WHERE id = $2")
                .bind(pm_id)
                .bind(intent_id)
                .execute(&self.pool)
                .await
                .map_err(|e| PaymentError::DatabaseError(format!("Failed to update payment method: {}", e)))?;
        }

        sqlx::query("UPDATE payment_intents SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(new_status.to_string())
            .bind(Utc::now())
            .bind(intent_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to confirm payment intent: {}", e)))?;

        // If not requiring action, process payment
        if !requires_action {
            // Simulate payment processing
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            sqlx::query("UPDATE payment_intents SET status = $1, updated_at = $2 WHERE id = $3")
                .bind(PaymentIntentStatus::Succeeded.to_string())
                .bind(Utc::now())
                .bind(intent_id)
                .execute(&self.pool)
                .await
                .map_err(|e| PaymentError::DatabaseError(format!("Failed to update status: {}", e)))?;
        }

        self.get_payment_intent(intent_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Payment Intent not found after confirmation".to_string()))
    }

    /// Handles 3D Secure authentication
    pub async fn handle_3d_secure(
        &self,
        intent_id: Uuid,
        authentication_result: bool,
    ) -> Result<PaymentIntent, PaymentError> {
        let intent = self.get_payment_intent(intent_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Payment Intent not found".to_string()))?;

        if intent.status != PaymentIntentStatus::RequiresAction {
            return Err(PaymentError::DatabaseError("Payment Intent does not require action".to_string()));
        }

        let new_status = if authentication_result {
            PaymentIntentStatus::Processing
        } else {
            PaymentIntentStatus::Canceled
        };

        sqlx::query("UPDATE payment_intents SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(new_status.to_string())
            .bind(Utc::now())
            .bind(intent_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to handle 3DS: {}", e)))?;

        // If authenticated, process payment
        if authentication_result {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            sqlx::query("UPDATE payment_intents SET status = $1, updated_at = $2 WHERE id = $3")
                .bind(PaymentIntentStatus::Succeeded.to_string())
                .bind(Utc::now())
                .bind(intent_id)
                .execute(&self.pool)
                .await
                .map_err(|e| PaymentError::DatabaseError(format!("Failed to process payment: {}", e)))?;
        }

        self.get_payment_intent(intent_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Payment Intent not found after 3DS".to_string()))
    }

    /// Gets a Payment Intent by ID
    pub async fn get_payment_intent(&self, intent_id: Uuid) -> Result<Option<PaymentIntent>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, customer_id, payment_method_id, amount, currency, status,
                   confirmation_method, client_secret, metadata, created_at, updated_at
            FROM payment_intents
            WHERE id = $1
            "#
        )
        .bind(intent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get payment intent: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_payment_intent(row)?)),
            None => Ok(None),
        }
    }

    /// Gets a Payment Intent by client secret
    pub async fn get_payment_intent_by_secret(
        &self,
        client_secret: &str,
    ) -> Result<Option<PaymentIntent>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, customer_id, payment_method_id, amount, currency, status,
                   confirmation_method, client_secret, metadata, created_at, updated_at
            FROM payment_intents
            WHERE client_secret = $1
            "#
        )
        .bind(client_secret)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get payment intent: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_payment_intent(row)?)),
            None => Ok(None),
        }
    }

    /// Cancels a Payment Intent
    pub async fn cancel_payment_intent(&self, intent_id: Uuid) -> Result<PaymentIntent, PaymentError> {
        sqlx::query("UPDATE payment_intents SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(PaymentIntentStatus::Canceled.to_string())
            .bind(Utc::now())
            .bind(intent_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to cancel payment intent: {}", e)))?;

        self.get_payment_intent(intent_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Payment Intent not found after cancelation".to_string()))
    }

    /// Determines if 3D Secure is required (simplified logic)
    fn requires_3d_secure(intent: &PaymentIntent) -> bool {
        // In production, this would check:
        // - Card issuer requirements
        // - Transaction amount thresholds
        // - Customer location
        // - PSD2 SCA requirements
        
        // For now, require 3DS for amounts over $50
        intent.amount > 5000
    }

    fn row_to_payment_intent(row: sqlx::postgres::PgRow) -> Result<PaymentIntent, PaymentError> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "REQUIRES_PAYMENT_METHOD" => PaymentIntentStatus::RequiresPaymentMethod,
            "REQUIRES_CONFIRMATION" => PaymentIntentStatus::RequiresConfirmation,
            "REQUIRES_ACTION" => PaymentIntentStatus::RequiresAction,
            "PROCESSING" => PaymentIntentStatus::Processing,
            "REQUIRES_CAPTURE" => PaymentIntentStatus::RequiresCapture,
            "CANCELED" => PaymentIntentStatus::Canceled,
            "SUCCEEDED" => PaymentIntentStatus::Succeeded,
            _ => return Err(PaymentError::DatabaseError("Invalid payment intent status".to_string())),
        };

        let confirmation_str: String = row.get("confirmation_method");
        let confirmation_method = match confirmation_str.as_str() {
            "automatic" => ConfirmationMethod::Automatic,
            "manual" => ConfirmationMethod::Manual,
            _ => ConfirmationMethod::Automatic,
        };

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(PaymentIntent {
            id: row.get("id"),
            customer_id: row.get("customer_id"),
            payment_method_id: row.get("payment_method_id"),
            amount: row.get("amount"),
            currency: row.get("currency"),
            status,
            confirmation_method,
            client_secret: row.get("client_secret"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

impl std::fmt::Display for PaymentIntentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentIntentStatus::RequiresPaymentMethod => write!(f, "REQUIRES_PAYMENT_METHOD"),
            PaymentIntentStatus::RequiresConfirmation => write!(f, "REQUIRES_CONFIRMATION"),
            PaymentIntentStatus::RequiresAction => write!(f, "REQUIRES_ACTION"),
            PaymentIntentStatus::Processing => write!(f, "PROCESSING"),
            PaymentIntentStatus::RequiresCapture => write!(f, "REQUIRES_CAPTURE"),
            PaymentIntentStatus::Canceled => write!(f, "CANCELED"),
            PaymentIntentStatus::Succeeded => write!(f, "SUCCEEDED"),
        }
    }
}

impl std::fmt::Display for ConfirmationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfirmationMethod::Automatic => write!(f, "automatic"),
            ConfirmationMethod::Manual => write!(f, "manual"),
        }
    }
}

