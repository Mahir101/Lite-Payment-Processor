//! # Webhook Service
//! 
//! This module handles webhook management, delivery, and signature verification.

use shared::{Webhook, WebhookEvent, WebhookEventStatus, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

pub struct WebhookService {
    pool: PgPool,
}

impl WebhookService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new webhook endpoint
    pub async fn create_webhook(
        &self,
        url: String,
        events: Vec<String>,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<Webhook, PaymentError> {
        let webhook_id = Uuid::new_v4();
        let secret_key = Self::generate_secret_key();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO webhooks (id, url, secret_key, events, active, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(webhook_id)
        .bind(&url)
        .bind(&secret_key)
        .bind(&events)
        .bind(true)
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create webhook: {}", e)))?;

        Ok(Webhook {
            id: webhook_id,
            url,
            secret_key,
            events,
            active: true,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Gets a webhook by ID
    pub async fn get_webhook(&self, webhook_id: Uuid) -> Result<Option<Webhook>, PaymentError> {
        let row = sqlx::query(
            "SELECT id, url, secret_key, events, active, metadata, created_at, updated_at FROM webhooks WHERE id = $1"
        )
        .bind(webhook_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get webhook: {}", e)))?;

        match row {
            Some(row) => {
                let events: Vec<String> = row.get("events");
                let metadata_value: serde_json::Value = row.get("metadata");
                let metadata: HashMap<String, String> = serde_json::from_value(metadata_value)
                    .unwrap_or_default();

                Ok(Some(Webhook {
                    id: row.get("id"),
                    url: row.get("url"),
                    secret_key: row.get("secret_key"),
                    events,
                    active: row.get("active"),
                    metadata,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Lists all active webhooks
    pub async fn list_webhooks(&self) -> Result<Vec<Webhook>, PaymentError> {
        let rows = sqlx::query(
            "SELECT id, url, secret_key, events, active, metadata, created_at, updated_at FROM webhooks WHERE active = TRUE ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list webhooks: {}", e)))?;

        let mut webhooks = Vec::new();
        for row in rows {
            let events: Vec<String> = row.get("events");
            let metadata_value: serde_json::Value = row.get("metadata");
            let metadata: HashMap<String, String> = serde_json::from_value(metadata_value)
                .unwrap_or_default();

            webhooks.push(Webhook {
                id: row.get("id"),
                url: row.get("url"),
                secret_key: row.get("secret_key"),
                events,
                active: row.get("active"),
                metadata,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(webhooks)
    }

    /// Delivers a webhook event to all registered webhooks
    pub async fn deliver_event(
        &self,
        event_type: String,
        event_data: serde_json::Value,
    ) -> Result<(), PaymentError> {
        // Get all active webhooks that subscribe to this event type
        let webhooks = self.list_webhooks().await?;
        
        for webhook in webhooks {
            // Check if webhook subscribes to this event type
            if webhook.events.contains(&event_type) || webhook.events.contains(&"*".to_string()) {
                // Create webhook event record
                let event_id = Uuid::new_v4();
                
                sqlx::query(
                    r#"
                    INSERT INTO webhook_events (
                        id, webhook_id, event_type, event_data, status, attempts, created_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#
                )
                .bind(event_id)
                .bind(webhook.id)
                .bind(&event_type)
                .bind(&event_data)
                .bind(WebhookEventStatus::Pending.to_string())
                .bind(0i32)
                .bind(Utc::now())
                .execute(&self.pool)
                .await
                .map_err(|e| PaymentError::DatabaseError(format!("Failed to create webhook event: {}", e)))?;

                // Deliver webhook asynchronously (in production, use a background job queue)
                let webhook_clone = webhook.clone();
                let event_data_clone = event_data.clone();
                let pool_clone = self.pool.clone();
                
                tokio::spawn(async move {
                    Self::deliver_webhook_http(webhook_clone, event_type.clone(), event_data_clone, pool_clone).await;
                });
            }
        }

        Ok(())
    }

    /// Delivers a webhook via HTTP POST
    async fn deliver_webhook_http(
        webhook: Webhook,
        event_type: String,
        event_data: serde_json::Value,
        pool: PgPool,
    ) {
        let payload = serde_json::json!({
            "id": Uuid::new_v4(),
            "type": event_type,
            "data": event_data,
            "created": Utc::now().timestamp(),
        });

        // Generate signature
        let signature = Self::generate_signature(&webhook.secret_key, &payload.to_string());

        // Send HTTP POST request
        let client = reqwest::Client::new();
        let response = client
            .post(&webhook.url)
            .header("X-Webhook-Signature", signature)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await;

        // Update webhook event status
        let status = match response {
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                
                if status_code >= 200 && status_code < 300 {
                    WebhookEventStatus::Delivered
                } else {
                    WebhookEventStatus::Failed
                }
            }
            Err(_) => WebhookEventStatus::Failed,
        };

        // Update event record (simplified - in production, track attempt count)
        // This is a simplified version - in production you'd want retry logic
    }

    /// Verifies webhook signature
    pub fn verify_signature(secret_key: &str, payload: &str, signature: &str) -> bool {
        let expected_signature = Self::generate_signature(secret_key, payload);
        expected_signature == signature
    }

    /// Generates HMAC signature for webhook
    fn generate_signature(secret_key: &str, payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

    /// Generates a secret key for webhook
    fn generate_secret_key() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        hex::encode(bytes)
    }
}

impl std::fmt::Display for WebhookEventStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookEventStatus::Pending => write!(f, "PENDING"),
            WebhookEventStatus::Delivered => write!(f, "DELIVERED"),
            WebhookEventStatus::Failed => write!(f, "FAILED"),
        }
    }
}

