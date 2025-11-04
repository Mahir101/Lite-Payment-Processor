//! # Tax Service
//! 
//! This module handles tax calculation, tax rates, and compliance.

use shared::{TaxRate, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;

pub struct TaxService {
    pool: PgPool,
}

impl TaxService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Calculates tax for an amount
    pub async fn calculate_tax(
        &self,
        amount: i64,
        country: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Result<TaxCalculation, PaymentError> {
        // Get applicable tax rate
        let tax_rate = self.get_applicable_tax_rate(country, jurisdiction).await?;
        
        let tax_amount = if tax_rate.inclusive {
            // Tax is included in amount, calculate backwards
            let total = amount as f64;
            let subtotal = total / (1.0 + tax_rate.percentage / 100.0);
            (total - subtotal).round() as i64
        } else {
            // Tax is added to amount
            ((amount as f64) * tax_rate.percentage / 100.0).round() as i64
        };

        Ok(TaxCalculation {
            subtotal: if tax_rate.inclusive { amount - tax_amount } else { amount },
            tax_amount,
            total: if tax_rate.inclusive { amount } else { amount + tax_amount },
            tax_rate: tax_rate.clone(),
        })
    }

    /// Gets applicable tax rate for a location
    pub async fn get_applicable_tax_rate(
        &self,
        country: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Result<TaxRate, PaymentError> {
        let query = match (country, jurisdiction) {
            (Some(country), Some(jurisdiction)) => {
                sqlx::query(
                    r#"
                    SELECT id, display_name, percentage, inclusive, country, jurisdiction, active, metadata, created_at, updated_at
                    FROM tax_rates
                    WHERE active = TRUE AND country = $1 AND jurisdiction = $2
                    ORDER BY created_at DESC
                    LIMIT 1
                    "#
                )
                .bind(country)
                .bind(jurisdiction)
            }
            (Some(country), None) => {
                sqlx::query(
                    r#"
                    SELECT id, display_name, percentage, inclusive, country, jurisdiction, active, metadata, created_at, updated_at
                    FROM tax_rates
                    WHERE active = TRUE AND country = $1
                    ORDER BY created_at DESC
                    LIMIT 1
                    "#
                )
                .bind(country)
            }
            _ => {
                // Default tax rate (0%)
                return Ok(TaxRate {
                    id: Uuid::new_v4(),
                    display_name: "No Tax".to_string(),
                    percentage: 0.0,
                    inclusive: false,
                    country: None,
                    jurisdiction: None,
                    active: true,
                    metadata: std::collections::HashMap::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                });
            }
        }
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get tax rate: {}", e)))?;

        match query {
            Some(row) => Ok(Self::row_to_tax_rate(row)?),
            None => {
                // Default to no tax
                Ok(TaxRate {
                    id: Uuid::new_v4(),
                    display_name: "No Tax".to_string(),
                    percentage: 0.0,
                    inclusive: false,
                    country: country.map(|s| s.to_string()),
                    jurisdiction: jurisdiction.map(|s| s.to_string()),
                    active: true,
                    metadata: std::collections::HashMap::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                })
            }
        }
    }

    /// Creates a new tax rate
    pub async fn create_tax_rate(
        &self,
        display_name: String,
        percentage: f64,
        inclusive: bool,
        country: Option<String>,
        jurisdiction: Option<String>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<TaxRate, PaymentError> {
        let tax_rate_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO tax_rates (
                id, display_name, percentage, inclusive, country, jurisdiction,
                active, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(tax_rate_id)
        .bind(&display_name)
        .bind(percentage)
        .bind(inclusive)
        .bind(country.as_deref())
        .bind(jurisdiction.as_deref())
        .bind(true)
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create tax rate: {}", e)))?;

        Ok(TaxRate {
            id: tax_rate_id,
            display_name,
            percentage,
            inclusive,
            country,
            jurisdiction,
            active: true,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Lists tax rates
    pub async fn list_tax_rates(
        &self,
        country: Option<&str>,
        active_only: bool,
    ) -> Result<Vec<TaxRate>, PaymentError> {
        let query = if let Some(country) = country {
            if active_only {
                sqlx::query(
                    r#"
                    SELECT id, display_name, percentage, inclusive, country, jurisdiction, active, metadata, created_at, updated_at
                    FROM tax_rates
                    WHERE country = $1 AND active = TRUE
                    ORDER BY created_at DESC
                    "#
                )
                .bind(country)
            } else {
                sqlx::query(
                    r#"
                    SELECT id, display_name, percentage, inclusive, country, jurisdiction, active, metadata, created_at, updated_at
                    FROM tax_rates
                    WHERE country = $1
                    ORDER BY created_at DESC
                    "#
                )
                .bind(country)
            }
        } else if active_only {
            sqlx::query(
                r#"
                SELECT id, display_name, percentage, inclusive, country, jurisdiction, active, metadata, created_at, updated_at
                FROM tax_rates
                WHERE active = TRUE
                ORDER BY created_at DESC
                "#
            )
        } else {
            sqlx::query(
                r#"
                SELECT id, display_name, percentage, inclusive, country, jurisdiction, active, metadata, created_at, updated_at
                FROM tax_rates
                ORDER BY created_at DESC
                "#
            )
        }
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list tax rates: {}", e)))?;

        let mut tax_rates = Vec::new();
        for row in query {
            tax_rates.push(Self::row_to_tax_rate(row)?);
        }

        Ok(tax_rates)
    }

    fn row_to_tax_rate(row: sqlx::postgres::PgRow) -> Result<TaxRate, PaymentError> {
        let percentage: f64 = row.get::<rust_decimal::Decimal, _>("percentage").to_string().parse::<f64>().unwrap_or(0.0);
        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(TaxRate {
            id: row.get("id"),
            display_name: row.get("display_name"),
            percentage,
            inclusive: row.get("inclusive"),
            country: row.get("country"),
            jurisdiction: row.get("jurisdiction"),
            active: row.get("active"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct TaxCalculation {
    pub subtotal: i64,
    pub tax_amount: i64,
    pub total: i64,
    pub tax_rate: TaxRate,
}

