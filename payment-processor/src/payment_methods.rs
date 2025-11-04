//! # Payment Method Service
//! 
//! This module handles payment method management with PCI-compliant tokenization.
//! Card numbers are never stored - only tokens are used.

use shared::{PaymentMethod, PaymentMethodType, CardBrand, BillingAddress, CardInfo, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;
use sha2::{Sha256, Digest};

pub struct PaymentMethodService {
    pool: PgPool,
}

impl PaymentMethodService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Tokenizes a card and creates a payment method
    /// This is PCI-compliant - we never store the full card number
    pub async fn create_payment_method(
        &self,
        customer_id: Option<Uuid>,
        card_info: &CardInfo,
        r#type: PaymentMethodType,
        is_default: Option<bool>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<PaymentMethod, PaymentError> {
        // Tokenize the card (never store PAN)
        let card_token = Self::tokenize_card(&card_info.pan);
        
        // Extract card details
        let card_brand = Self::detect_card_brand(&card_info.pan);
        let card_last4 = card_info.pan.chars().rev().take(4).collect::<String>().chars().rev().collect::<String>();
        
        // If this is set as default, unset other defaults for this customer
        if let Some(customer_id) = customer_id {
            if is_default.unwrap_or(false) {
                sqlx::query("UPDATE payment_methods SET is_default = FALSE WHERE customer_id = $1")
                    .bind(customer_id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| PaymentError::DatabaseError(format!("Failed to update defaults: {}", e)))?;
            }
        }

        let payment_method_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO payment_methods (
                id, customer_id, type, card_token, card_brand, card_last4,
                card_exp_month, card_exp_year, cardholder_name, billing_address,
                is_default, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#
        )
        .bind(payment_method_id)
        .bind(customer_id)
        .bind(r#type.to_string())
        .bind(&card_token)
        .bind(card_brand.as_ref().map(|b| b.to_string()))
        .bind(&card_last4)
        .bind(card_info.expiry_month as i32)
        .bind(card_info.expiry_year as i32)
        .bind(&card_info.cardholder_name)
        .bind(serde_json::to_value(&card_info.billing_address).unwrap())
        .bind(is_default.unwrap_or(false))
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create payment method: {}", e)))?;

        Ok(PaymentMethod {
            id: payment_method_id,
            customer_id,
            r#type,
            card_token,
            card_brand,
            card_last4: Some(card_last4),
            card_exp_month: Some(card_info.expiry_month),
            card_exp_year: Some(card_info.expiry_year),
            cardholder_name: Some(card_info.cardholder_name.clone()),
            billing_address: Some(card_info.billing_address.clone()),
            is_default: is_default.unwrap_or(false),
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Gets a payment method by ID
    pub async fn get_payment_method(&self, payment_method_id: Uuid) -> Result<Option<PaymentMethod>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, customer_id, type, card_token, card_brand, card_last4,
                   card_exp_month, card_exp_year, cardholder_name, billing_address,
                   is_default, metadata, created_at, updated_at
            FROM payment_methods
            WHERE id = $1
            "#
        )
        .bind(payment_method_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get payment method: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_payment_method(row)?)),
            None => Ok(None),
        }
    }

    /// Lists payment methods for a customer
    pub async fn list_payment_methods(
        &self,
        customer_id: Option<Uuid>,
    ) -> Result<Vec<PaymentMethod>, PaymentError> {
        let query = if let Some(customer_id) = customer_id {
            sqlx::query(
                r#"
                SELECT id, customer_id, type, card_token, card_brand, card_last4,
                       card_exp_month, card_exp_year, cardholder_name, billing_address,
                       is_default, metadata, created_at, updated_at
                FROM payment_methods
                WHERE customer_id = $1
                ORDER BY is_default DESC, created_at DESC
                "#
            )
            .bind(customer_id)
        } else {
            sqlx::query(
                r#"
                SELECT id, customer_id, type, card_token, card_brand, card_last4,
                       card_exp_month, card_exp_year, cardholder_name, billing_address,
                       is_default, metadata, created_at, updated_at
                FROM payment_methods
                ORDER BY created_at DESC
                "#
            )
        }
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list payment methods: {}", e)))?;

        let mut payment_methods = Vec::new();
        for row in query {
            payment_methods.push(Self::row_to_payment_method(row)?);
        }

        Ok(payment_methods)
    }

    /// Sets a payment method as default for a customer
    pub async fn set_default_payment_method(
        &self,
        customer_id: Uuid,
        payment_method_id: Uuid,
    ) -> Result<(), PaymentError> {
        // Start transaction
        let mut tx = self.pool.begin().await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to start transaction: {}", e)))?;

        // Unset all defaults for this customer
        sqlx::query("UPDATE payment_methods SET is_default = FALSE WHERE customer_id = $1")
            .bind(customer_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to update defaults: {}", e)))?;

        // Set this payment method as default
        sqlx::query("UPDATE payment_methods SET is_default = TRUE WHERE id = $1 AND customer_id = $2")
            .bind(payment_method_id)
            .bind(customer_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to set default: {}", e)))?;

        tx.commit().await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to commit: {}", e)))?;

        Ok(())
    }

    /// Deletes a payment method
    pub async fn delete_payment_method(&self, payment_method_id: Uuid) -> Result<(), PaymentError> {
        sqlx::query("DELETE FROM payment_methods WHERE id = $1")
            .bind(payment_method_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to delete payment method: {}", e)))?;

        Ok(())
    }

    /// Tokenizes a card number (PCI-compliant)
    /// Uses SHA-256 hash with salt as token
    fn tokenize_card(pan: &str) -> String {
        // In production, use a proper tokenization service
        // For now, we'll use a hash-based approach
        let mut hasher = Sha256::new();
        hasher.update(pan.as_bytes());
        hasher.update(Uuid::new_v4().to_string().as_bytes()); // Add salt
        format!("card_{:x}", hasher.finalize())
    }

    /// Detects card brand from PAN
    fn detect_card_brand(pan: &str) -> Option<CardBrand> {
        let cleaned: String = pan.chars().filter(|c| c.is_ascii_digit()).collect();
        
        if cleaned.starts_with("4") {
            Some(CardBrand::Visa)
        } else if cleaned.starts_with("5") && cleaned.len() >= 2 {
            let second = cleaned.chars().nth(1).unwrap();
            if second >= '1' && second <= '5' {
                Some(CardBrand::Mastercard)
            } else {
                Some(CardBrand::Unknown)
            }
        } else if cleaned.starts_with("3") && cleaned.len() >= 2 {
            let second = cleaned.chars().nth(1).unwrap();
            if second == '4' || second == '7' {
                Some(CardBrand::Amex)
            } else if second == '0' || second == '6' || second == '8' || second == '9' {
                Some(CardBrand::DinersClub)
            } else {
                Some(CardBrand::Unknown)
            }
        } else if cleaned.starts_with("6") {
            Some(CardBrand::Discover)
        } else {
            Some(CardBrand::Unknown)
        }
    }

    /// Converts database row to PaymentMethod
    fn row_to_payment_method(row: sqlx::postgres::PgRow) -> Result<PaymentMethod, PaymentError> {
        let type_str: String = row.get("type");
        let r#type = match type_str.as_str() {
            "card" => PaymentMethodType::Card,
            "ach" => PaymentMethodType::Ach,
            "bank_account" => PaymentMethodType::BankAccount,
            "paypal" => PaymentMethodType::Paypal,
            "apple_pay" => PaymentMethodType::ApplePay,
            "google_pay" => PaymentMethodType::GooglePay,
            _ => return Err(PaymentError::DatabaseError("Invalid payment method type".to_string())),
        };

        let card_brand_str: Option<String> = row.get("card_brand");
        let card_brand = card_brand_str.as_deref().and_then(|b| match b {
            "Visa" => Some(CardBrand::Visa),
            "Mastercard" => Some(CardBrand::Mastercard),
            "Amex" => Some(CardBrand::Amex),
            "Discover" => Some(CardBrand::Discover),
            "Jcb" => Some(CardBrand::Jcb),
            "DinersClub" => Some(CardBrand::DinersClub),
            "UnionPay" => Some(CardBrand::UnionPay),
            _ => Some(CardBrand::Unknown),
        });

        let billing_address_value: Option<serde_json::Value> = row.get("billing_address");
        let billing_address = billing_address_value.and_then(|v| serde_json::from_value(v).ok());

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(PaymentMethod {
            id: row.get("id"),
            customer_id: row.get("customer_id"),
            r#type,
            card_token: row.get("card_token"),
            card_brand,
            card_last4: row.get("card_last4"),
            card_exp_month: row.get::<Option<i32>, _>("card_exp_month").map(|m| m as u8),
            card_exp_year: row.get::<Option<i32>, _>("card_exp_year").map(|y| y as u16),
            cardholder_name: row.get("cardholder_name"),
            billing_address,
            is_default: row.get("is_default"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

impl std::fmt::Display for PaymentMethodType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentMethodType::Card => write!(f, "card"),
            PaymentMethodType::Ach => write!(f, "ach"),
            PaymentMethodType::BankAccount => write!(f, "bank_account"),
            PaymentMethodType::Paypal => write!(f, "paypal"),
            PaymentMethodType::ApplePay => write!(f, "apple_pay"),
            PaymentMethodType::GooglePay => write!(f, "google_pay"),
        }
    }
}

impl std::fmt::Display for CardBrand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardBrand::Visa => write!(f, "Visa"),
            CardBrand::Mastercard => write!(f, "Mastercard"),
            CardBrand::Amex => write!(f, "Amex"),
            CardBrand::Discover => write!(f, "Discover"),
            CardBrand::Jcb => write!(f, "Jcb"),
            CardBrand::DinersClub => write!(f, "DinersClub"),
            CardBrand::UnionPay => write!(f, "UnionPay"),
            CardBrand::Unknown => write!(f, "Unknown"),
        }
    }
}

