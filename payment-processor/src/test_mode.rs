//! # Test Mode Service
//! 
//! This module handles test mode configuration, test card numbers,
//! and sandbox environment management.

use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

pub struct TestModeService {
    pool: PgPool,
}

impl TestModeService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Checks if test mode is enabled
    pub async fn is_test_mode_enabled(&self) -> bool {
        let row = sqlx::query("SELECT test_mode_enabled FROM test_configurations ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();

        row.map(|r| r.get::<bool, _>("test_mode_enabled")).unwrap_or(false)
    }

    /// Gets test card numbers
    pub async fn get_test_cards(&self) -> HashMap<String, TestCardInfo> {
        let row = sqlx::query("SELECT test_card_numbers FROM test_configurations ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();

        if let Some(row) = row {
            let cards_json: serde_json::Value = row.get("test_card_numbers");
            if let Ok(cards) = serde_json::from_value(cards_json) {
                return cards;
            }
        }

        // Default test cards
        let mut default_cards = HashMap::new();
        
        default_cards.insert("4242424242424242".to_string(), TestCardInfo {
            brand: "Visa".to_string(),
            success: true,
            description: "Visa test card (always succeeds)".to_string(),
        });
        
        default_cards.insert("4000000000000002".to_string(), TestCardInfo {
            brand: "Visa".to_string(),
            success: false,
            description: "Visa test card (always declined)".to_string(),
        });
        
        default_cards.insert("4000000000009995".to_string(), TestCardInfo {
            brand: "Visa".to_string(),
            success: false,
            description: "Visa test card (insufficient funds)".to_string(),
        });

        default_cards
    }

    /// Validates if a card is a test card
    pub async fn is_test_card(&self, pan: &str) -> bool {
        let test_cards = self.get_test_cards().await;
        test_cards.contains_key(pan)
    }

    /// Enables or disables test mode
    pub async fn set_test_mode(&self, enabled: bool) -> Result<(), sqlx::Error> {
        let config_id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO test_configurations (id, test_mode_enabled, test_card_numbers, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO UPDATE
            SET test_mode_enabled = $2, updated_at = $5
            "#
        )
        .bind(config_id)
        .bind(enabled)
        .bind(serde_json::to_value(self.get_test_cards().await).unwrap_or_default())
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Gets test webhook URLs
    pub async fn get_test_webhook_urls(&self) -> Vec<String> {
        let row = sqlx::query("SELECT test_webhook_urls FROM test_configurations ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();

        if let Some(row) = row {
            let urls: Option<Vec<String>> = row.get("test_webhook_urls");
            if let Some(urls) = urls {
                return urls;
            }
        }

        vec!["https://webhook.site/test".to_string()]
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestCardInfo {
    pub brand: String,
    pub success: bool,
    pub description: String,
}

