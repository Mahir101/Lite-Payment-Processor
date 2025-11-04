//! # Currency Service
//! 
//! This module handles multi-currency support, exchange rates,
//! and currency conversion.

use shared::{ExchangeRate, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{Utc, NaiveDate};
use std::collections::HashMap;

pub struct CurrencyService {
    pool: PgPool,
}

impl CurrencyService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Converts amount from one currency to another
    pub async fn convert_currency(
        &self,
        amount: i64,
        from_currency: &str,
        to_currency: &str,
    ) -> Result<i64, PaymentError> {
        if from_currency == to_currency {
            return Ok(amount);
        }

        // Get exchange rate
        let rate = self.get_exchange_rate(from_currency, to_currency).await?;
        
        // Convert amount (amount is in cents, so we need to handle precision)
        let converted = (amount as f64 * rate.rate).round() as i64;
        
        Ok(converted)
    }

    /// Gets exchange rate for currency pair
    pub async fn get_exchange_rate(
        &self,
        base_currency: &str,
        target_currency: &str,
    ) -> Result<ExchangeRate, PaymentError> {
        let today = Utc::now().date_naive();
        
        // Try to get today's rate first
        let row = sqlx::query(
            r#"
            SELECT id, base_currency, target_currency, rate, effective_date, created_at
            FROM exchange_rates
            WHERE base_currency = $1 AND target_currency = $2 AND effective_date = $3
            ORDER BY created_at DESC
            LIMIT 1
            "#
        )
        .bind(base_currency)
        .bind(target_currency)
        .bind(today)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get exchange rate: {}", e)))?;

        if let Some(row) = row {
            return Ok(ExchangeRate {
                id: row.get("id"),
                base_currency: row.get("base_currency"),
                target_currency: row.get("target_currency"),
                rate: row.get::<rust_decimal::Decimal, _>("rate").to_string().parse::<f64>().unwrap_or(1.0),
                effective_date: row.get("effective_date"),
                created_at: row.get("created_at"),
            });
        }

        // Fallback to most recent rate
        let row = sqlx::query(
            r#"
            SELECT id, base_currency, target_currency, rate, effective_date, created_at
            FROM exchange_rates
            WHERE base_currency = $1 AND target_currency = $2
            ORDER BY effective_date DESC, created_at DESC
            LIMIT 1
            "#
        )
        .bind(base_currency)
        .bind(target_currency)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get exchange rate: {}", e)))?;

        match row {
            Some(row) => Ok(ExchangeRate {
                id: row.get("id"),
                base_currency: row.get("base_currency"),
                target_currency: row.get("target_currency"),
                rate: row.get::<rust_decimal::Decimal, _>("rate").to_string().parse::<f64>().unwrap_or(1.0),
                effective_date: row.get("effective_date"),
                created_at: row.get("created_at"),
            }),
            None => {
                // If no rate found, create a default 1:1 rate (for same currency or missing rates)
                if base_currency == target_currency {
                    Ok(ExchangeRate {
                        id: Uuid::new_v4(),
                        base_currency: base_currency.to_string(),
                        target_currency: target_currency.to_string(),
                        rate: 1.0,
                        effective_date: today,
                        created_at: Utc::now(),
                    })
                } else {
                    Err(PaymentError::DatabaseError(format!(
                        "Exchange rate not found for {} to {}",
                        base_currency, target_currency
                    )))
                }
            }
        }
    }

    /// Updates or creates an exchange rate
    pub async fn set_exchange_rate(
        &self,
        base_currency: &str,
        target_currency: &str,
        rate: f64,
        effective_date: Option<NaiveDate>,
    ) -> Result<ExchangeRate, PaymentError> {
        let date = effective_date.unwrap_or_else(|| Utc::now().date_naive());
        let rate_id = Uuid::new_v4();

        // Check if rate already exists for this date
        let existing = sqlx::query(
            "SELECT id FROM exchange_rates WHERE base_currency = $1 AND target_currency = $2 AND effective_date = $3"
        )
        .bind(base_currency)
        .bind(target_currency)
        .bind(date)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to check existing rate: {}", e)))?;

        if let Some(row) = existing {
            // Update existing rate
            let existing_id: Uuid = row.get("id");
            sqlx::query(
                "UPDATE exchange_rates SET rate = $1 WHERE id = $2"
            )
            .bind(rust_decimal::Decimal::from_f64_retain(rate).unwrap_or(rust_decimal::Decimal::ONE))
            .bind(existing_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to update rate: {}", e)))?;

            Ok(ExchangeRate {
                id: existing_id,
                base_currency: base_currency.to_string(),
                target_currency: target_currency.to_string(),
                rate,
                effective_date: date,
                created_at: Utc::now(),
            })
        } else {
            // Create new rate
            sqlx::query(
                r#"
                INSERT INTO exchange_rates (id, base_currency, target_currency, rate, effective_date, created_at)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#
            )
            .bind(rate_id)
            .bind(base_currency)
            .bind(target_currency)
            .bind(rust_decimal::Decimal::from_f64_retain(rate).unwrap_or(rust_decimal::Decimal::ONE))
            .bind(date)
            .bind(Utc::now())
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to create rate: {}", e)))?;

            Ok(ExchangeRate {
                id: rate_id,
                base_currency: base_currency.to_string(),
                target_currency: target_currency.to_string(),
                rate,
                effective_date: date,
                created_at: Utc::now(),
            })
        }
    }

    /// Gets supported currencies
    pub fn get_supported_currencies() -> Vec<&'static str> {
        vec![
            "USD", "EUR", "GBP", "JPY", "AUD", "CAD", "CHF", "CNY",
            "INR", "SGD", "HKD", "NZD", "MXN", "BRL", "ZAR", "KRW",
        ]
    }
}

