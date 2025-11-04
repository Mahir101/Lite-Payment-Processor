//! # Subscription Service
//! 
//! This module handles subscription management, recurring billing,
//! and subscription lifecycle management.

use shared::{Subscription, SubscriptionStatus, SubscriptionItem, Product, Price, RecurringInterval, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{Utc, Duration};

pub struct SubscriptionService {
    pool: PgPool,
}

impl SubscriptionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new subscription
    pub async fn create_subscription(
        &self,
        customer_id: Uuid,
        price_id: Uuid,
        trial_days: Option<u32>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Subscription, PaymentError> {
        // Get price to determine billing interval
        let price = self.get_price(price_id).await?
            .ok_or_else(|| PaymentError::subscription_error("Price not found".to_string()))?;

        let subscription_id = Uuid::new_v4();
        let now = Utc::now();
        
        // Calculate trial period if provided
        let (trial_start, trial_end) = if let Some(days) = trial_days {
            (Some(now), Some(now + Duration::days(days as i64)))
        } else {
            (None, None)
        };

        // Calculate billing period
        let (period_start, period_end) = if trial_end.is_some() {
            (trial_end, Self::calculate_next_period_end(trial_end.unwrap(), &price))
        } else {
            (Some(now), Self::calculate_next_period_end(now, &price))
        };

        let status = if trial_end.is_some() {
            SubscriptionStatus::Trialing
        } else {
            SubscriptionStatus::Active
        };

        // Create subscription
        sqlx::query(
            r#"
            INSERT INTO subscriptions (
                id, customer_id, status, current_period_start, current_period_end,
                cancel_at_period_end, canceled_at, trial_start, trial_end,
                metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#
        )
        .bind(subscription_id)
        .bind(customer_id)
        .bind(status.to_string())
        .bind(period_start)
        .bind(period_end)
        .bind(false)
        .bind(None::<chrono::DateTime<Utc>>)
        .bind(trial_start)
        .bind(trial_end)
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create subscription: {}", e)))?;

        // Create subscription item
        let item_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO subscription_items (id, subscription_id, price_id, quantity, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(item_id)
        .bind(subscription_id)
        .bind(price_id)
        .bind(1i32)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create subscription item: {}", e)))?;

        Ok(Subscription {
            id: subscription_id,
            customer_id,
            status,
            current_period_start: period_start,
            current_period_end: period_end,
            cancel_at_period_end: false,
            canceled_at: None,
            trial_start,
            trial_end,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Gets a subscription by ID
    pub async fn get_subscription(&self, subscription_id: Uuid) -> Result<Option<Subscription>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, customer_id, status, current_period_start, current_period_end,
                   cancel_at_period_end, canceled_at, trial_start, trial_end,
                   metadata, created_at, updated_at
            FROM subscriptions
            WHERE id = $1
            "#
        )
        .bind(subscription_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get subscription: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_subscription(row)?)),
            None => Ok(None),
        }
    }

    /// Lists subscriptions for a customer
    pub async fn list_subscriptions(
        &self,
        customer_id: Option<Uuid>,
        status: Option<SubscriptionStatus>,
    ) -> Result<Vec<Subscription>, PaymentError> {
        let query = match (customer_id, status) {
            (Some(cid), Some(st)) => {
                sqlx::query(
                    r#"
                    SELECT id, customer_id, status, current_period_start, current_period_end,
                           cancel_at_period_end, canceled_at, trial_start, trial_end,
                           metadata, created_at, updated_at
                    FROM subscriptions
                    WHERE customer_id = $1 AND status = $2
                    ORDER BY created_at DESC
                    "#
                )
                .bind(cid)
                .bind(st.to_string())
            }
            (Some(cid), None) => {
                sqlx::query(
                    r#"
                    SELECT id, customer_id, status, current_period_start, current_period_end,
                           cancel_at_period_end, canceled_at, trial_start, trial_end,
                           metadata, created_at, updated_at
                    FROM subscriptions
                    WHERE customer_id = $1
                    ORDER BY created_at DESC
                    "#
                )
                .bind(cid)
            }
            (None, Some(st)) => {
                sqlx::query(
                    r#"
                    SELECT id, customer_id, status, current_period_start, current_period_end,
                           cancel_at_period_end, canceled_at, trial_start, trial_end,
                           metadata, created_at, updated_at
                    FROM subscriptions
                    WHERE status = $1
                    ORDER BY created_at DESC
                    "#
                )
                .bind(st.to_string())
            }
            (None, None) => {
                sqlx::query(
                    r#"
                    SELECT id, customer_id, status, current_period_start, current_period_end,
                           cancel_at_period_end, canceled_at, trial_start, trial_end,
                           metadata, created_at, updated_at
                    FROM subscriptions
                    ORDER BY created_at DESC
                    "#
                )
            }
        }
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list subscriptions: {}", e)))?;

        let mut subscriptions = Vec::new();
        for row in query {
            subscriptions.push(Self::row_to_subscription(row)?);
        }

        Ok(subscriptions)
    }

    /// Cancels a subscription
    pub async fn cancel_subscription(
        &self,
        subscription_id: Uuid,
        at_period_end: bool,
    ) -> Result<Subscription, PaymentError> {
        let subscription = self.get_subscription(subscription_id).await?
            .ok_or_else(|| PaymentError::subscription_error("Subscription not found".to_string()))?;

        if subscription.status == SubscriptionStatus::Canceled {
            return Err(PaymentError::subscription_error("Subscription already canceled".to_string()));
        }

        let canceled_at = if at_period_end {
            None
        } else {
            Some(Utc::now())
        };

        let new_status = if at_period_end {
            subscription.status // Keep current status until period ends
        } else {
            SubscriptionStatus::Canceled
        };

        sqlx::query(
            r#"
            UPDATE subscriptions
            SET status = $1, cancel_at_period_end = $2, canceled_at = $3, updated_at = $4
            WHERE id = $5
            "#
        )
        .bind(new_status.to_string())
        .bind(at_period_end)
        .bind(canceled_at)
        .bind(Utc::now())
        .bind(subscription_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to cancel subscription: {}", e)))?;

        Ok(Subscription {
            status: new_status,
            cancel_at_period_end: at_period_end,
            canceled_at,
            ..subscription
        })
    }

    /// Processes recurring billing (called by scheduler)
    pub async fn process_recurring_billing(&self) -> Result<(), PaymentError> {
        // Get all active subscriptions that need billing
        let now = Utc::now();
        
        let rows = sqlx::query(
            r#"
            SELECT s.id, s.customer_id, si.price_id
            FROM subscriptions s
            JOIN subscription_items si ON s.id = si.subscription_id
            WHERE s.status IN ('ACTIVE', 'TRIALING')
            AND s.current_period_end <= $1
            AND s.cancel_at_period_end = FALSE
            "#
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get subscriptions: {}", e)))?;

        for row in rows {
            let subscription_id: Uuid = row.get("id");
            let customer_id: Uuid = row.get("customer_id");
            let price_id: Uuid = row.get("price_id");

            // Get price
            let price = self.get_price(price_id).await?
                .ok_or_else(|| PaymentError::subscription_error("Price not found".to_string()))?;

            // Create invoice and charge (simplified - in production this would be more complex)
            // For now, we'll just update the subscription period
            let next_period_end = Self::calculate_next_period_end(now, &price);
            
            sqlx::query(
                "UPDATE subscriptions SET current_period_start = $1, current_period_end = $2, updated_at = $3 WHERE id = $4"
            )
            .bind(now)
            .bind(next_period_end)
            .bind(now)
            .bind(subscription_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to update subscription: {}", e)))?;
        }

        Ok(())
    }

    /// Gets a price by ID
    pub async fn get_price(&self, price_id: Uuid) -> Result<Option<Price>, PaymentError> {
        let row = sqlx::query(
            "SELECT id, product_id, amount, currency, recurring_interval, recurring_interval_count, active, metadata, created_at, updated_at FROM prices WHERE id = $1"
        )
        .bind(price_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get price: {}", e)))?;

        match row {
            Some(row) => {
                let interval_str: Option<String> = row.get("recurring_interval");
                let interval = interval_str.as_deref().and_then(|i| match i {
                    "day" => Some(RecurringInterval::Day),
                    "week" => Some(RecurringInterval::Week),
                    "month" => Some(RecurringInterval::Month),
                    "year" => Some(RecurringInterval::Year),
                    _ => None,
                });

                let metadata_value: serde_json::Value = row.get("metadata");
                let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
                    .unwrap_or_default();

                Ok(Some(Price {
                    id: row.get("id"),
                    product_id: row.get("product_id"),
                    amount: row.get("amount"),
                    currency: row.get("currency"),
                    recurring_interval: interval,
                    recurring_interval_count: row.get("recurring_interval_count"),
                    active: row.get("active"),
                    metadata,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Calculates next period end date
    fn calculate_next_period_end(start: chrono::DateTime<Utc>, price: &Price) -> Option<chrono::DateTime<Utc>> {
        if let Some(interval) = &price.recurring_interval {
            let count = price.recurring_interval_count.unwrap_or(1) as i64;
            Some(match interval {
                RecurringInterval::Day => start + Duration::days(count),
                RecurringInterval::Week => start + Duration::weeks(count),
                RecurringInterval::Month => {
                    // Simplified - in production use proper month arithmetic
                    start + Duration::days(count * 30)
                }
                RecurringInterval::Year => {
                    // Simplified - in production use proper year arithmetic
                    start + Duration::days(count * 365)
                }
            })
        } else {
            None
        }
    }

    /// Converts database row to Subscription
    fn row_to_subscription(row: sqlx::postgres::PgRow) -> Result<Subscription, PaymentError> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "INCOMPLETE" => SubscriptionStatus::Incomplete,
            "INCOMPLETE_EXPIRED" => SubscriptionStatus::IncompleteExpired,
            "TRIALING" => SubscriptionStatus::Trialing,
            "ACTIVE" => SubscriptionStatus::Active,
            "PAST_DUE" => SubscriptionStatus::PastDue,
            "CANCELED" => SubscriptionStatus::Canceled,
            "UNPAID" => SubscriptionStatus::Unpaid,
            "PAUSED" => SubscriptionStatus::Paused,
            _ => return Err(PaymentError::DatabaseError("Invalid subscription status".to_string())),
        };

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(Subscription {
            id: row.get("id"),
            customer_id: row.get("customer_id"),
            status,
            current_period_start: row.get("current_period_start"),
            current_period_end: row.get("current_period_end"),
            cancel_at_period_end: row.get("cancel_at_period_end"),
            canceled_at: row.get("canceled_at"),
            trial_start: row.get("trial_start"),
            trial_end: row.get("trial_end"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionStatus::Incomplete => write!(f, "INCOMPLETE"),
            SubscriptionStatus::IncompleteExpired => write!(f, "INCOMPLETE_EXPIRED"),
            SubscriptionStatus::Trialing => write!(f, "TRIALING"),
            SubscriptionStatus::Active => write!(f, "ACTIVE"),
            SubscriptionStatus::PastDue => write!(f, "PAST_DUE"),
            SubscriptionStatus::Canceled => write!(f, "CANCELED"),
            SubscriptionStatus::Unpaid => write!(f, "UNPAID"),
            SubscriptionStatus::Paused => write!(f, "PAUSED"),
        }
    }
}

impl std::fmt::Display for RecurringInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecurringInterval::Day => write!(f, "day"),
            RecurringInterval::Week => write!(f, "week"),
            RecurringInterval::Month => write!(f, "month"),
            RecurringInterval::Year => write!(f, "year"),
        }
    }
}

