//! # Advanced Fraud Detection Service
//! 
//! This module provides ML-based fraud detection with risk scoring,
//! behavioral analysis, and adaptive authentication.

use shared::{CardInfo, UserInfo, PaymentError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AdvancedFraudDetector {
    // Risk scoring thresholds
    high_risk_threshold: f64,
    medium_risk_threshold: f64,
    
    // Transaction history tracking
    transaction_history: Arc<RwLock<HashMap<String, Vec<TransactionRecord>>>>,
    
    // Device fingerprinting (simplified)
    device_fingerprints: Arc<RwLock<HashMap<String, DeviceProfile>>>,
}

#[derive(Clone)]
struct TransactionRecord {
    amount: i64,
    timestamp: chrono::DateTime<chrono::Utc>,
    location: Option<String>,
    success: bool,
}

#[derive(Clone)]
struct DeviceProfile {
    device_id: String,
    ip_address: Option<String>,
    user_agent: Option<String>,
    risk_score: f64,
    transaction_count: u32,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl AdvancedFraudDetector {
    pub fn new() -> Self {
        Self {
            high_risk_threshold: 0.7,
            medium_risk_threshold: 0.4,
            transaction_history: Arc::new(RwLock::new(HashMap::new())),
            device_fingerprints: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Analyzes a transaction and returns a risk score (0.0 to 1.0)
    pub async fn analyze_transaction(
        &self,
        card_info: &CardInfo,
        user_info: &Option<UserInfo>,
        amount: i64,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<FraudAnalysisResult, PaymentError> {
        let mut risk_score = 0.0;
        let mut risk_factors = Vec::new();

        // 1. Card velocity check (too many transactions in short time)
        let card_key = format!("card_{}", &card_info.pan[card_info.pan.len().saturating_sub(4)..]);
        let velocity_score = self.check_velocity(&card_key, amount).await;
        risk_score += velocity_score * 0.3;
        if velocity_score > 0.5 {
            risk_factors.push("High transaction velocity detected".to_string());
        }

        // 2. Amount anomaly detection
        let amount_score = self.check_amount_anomaly(&card_key, amount).await;
        risk_score += amount_score * 0.2;
        if amount_score > 0.5 {
            risk_factors.push("Unusual transaction amount".to_string());
        }

        // 3. Device fingerprinting
        if let Some(user_info) = user_info {
            let device_id = user_info.device_id.as_deref().unwrap_or("unknown");
            let device_score = self.analyze_device(device_id, ip_address, user_agent).await;
            risk_score += device_score * 0.2;
            if device_score > 0.5 {
                risk_factors.push("Unusual device or location".to_string());
            }
        }

        // 4. Card pattern analysis
        let card_pattern_score = self.analyze_card_pattern(card_info);
        risk_score += card_pattern_score * 0.15;
        if card_pattern_score > 0.5 {
            risk_factors.push("Suspicious card pattern".to_string());
        }

        // 5. User verification check
        if let Some(user_info) = user_info {
            if !user_info.is_verified {
                risk_score += 0.15;
                risk_factors.push("User not verified".to_string());
            }
        } else {
            risk_score += 0.1;
            risk_factors.push("No user information provided".to_string());
        }

        // Normalize risk score to 0.0-1.0
        risk_score = risk_score.min(1.0);

        let risk_level = if risk_score >= self.high_risk_threshold {
            RiskLevel::High
        } else if risk_score >= self.medium_risk_threshold {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        Ok(FraudAnalysisResult {
            risk_score,
            risk_level,
            risk_factors,
            should_block: risk_score >= self.high_risk_threshold,
            should_require_authentication: risk_score >= self.medium_risk_threshold,
        })
    }

    /// Checks transaction velocity (frequency of transactions)
    async fn check_velocity(&self, card_key: &str, amount: i64) -> f64 {
        let history = self.transaction_history.read().await;
        if let Some(transactions) = history.get(card_key) {
            let now = chrono::Utc::now();
            let recent_transactions: Vec<_> = transactions
                .iter()
                .filter(|t| (now - t.timestamp).num_minutes() < 60)
                .collect();

            if recent_transactions.len() > 10 {
                return 1.0; // Very high velocity
            } else if recent_transactions.len() > 5 {
                return 0.7; // High velocity
            } else if recent_transactions.len() > 2 {
                return 0.4; // Medium velocity
            }
        }
        0.0
    }

    /// Checks for amount anomalies (unusual transaction amounts)
    async fn check_amount_anomaly(&self, card_key: &str, amount: i64) -> f64 {
        let history = self.transaction_history.read().await;
        if let Some(transactions) = history.get(card_key) {
            if transactions.is_empty() {
                return 0.0;
            }

            // Calculate average transaction amount
            let avg_amount: f64 = transactions.iter()
                .map(|t| t.amount as f64)
                .sum::<f64>() / transactions.len() as f64;

            // Check if current amount is significantly different
            let deviation = (amount as f64 - avg_amount).abs() / avg_amount.max(1.0);
            
            if deviation > 3.0 {
                return 0.9; // Very unusual
            } else if deviation > 2.0 {
                return 0.6; // Unusual
            } else if deviation > 1.0 {
                return 0.3; // Somewhat unusual
            }
        }
        0.0
    }

    /// Analyzes device fingerprint
    async fn analyze_device(
        &self,
        device_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> f64 {
        let mut fingerprints = self.device_fingerprints.write().await;
        
        if let Some(profile) = fingerprints.get(device_id) {
            // Check if IP changed significantly
            if let Some(ip) = ip_address {
                if profile.ip_address.as_deref() != Some(ip) {
                    return 0.5; // IP changed
                }
            }
            
            // Check transaction count (too many from same device)
            if profile.transaction_count > 50 {
                return 0.4;
            }
            
            profile.risk_score
        } else {
            // New device - moderate risk
            let profile = DeviceProfile {
                device_id: device_id.to_string(),
                ip_address: ip_address.map(|s| s.to_string()),
                user_agent: user_agent.map(|s| s.to_string()),
                risk_score: 0.3,
                transaction_count: 0,
                created_at: chrono::Utc::now(),
            };
            fingerprints.insert(device_id.to_string(), profile);
            0.3
        }
    }

    /// Analyzes card patterns
    fn analyze_card_pattern(&self, card_info: &CardInfo) -> f64 {
        // Check for test card patterns
        let pan = &card_info.pan;
        let last4: String = pan.chars().rev().take(4).collect::<String>().chars().rev().collect();
        
        // Simple pattern checks
        if last4 == "0000" || last4 == "1111" || last4 == "1234" {
            return 0.8; // Suspicious pattern
        }
        
        // Check for sequential numbers
        let digits: Vec<u32> = last4.chars()
            .filter_map(|c| c.to_digit(10))
            .collect();
        
        if digits.len() == 4 {
            let is_sequential = (digits[0] + 1 == digits[1] && digits[1] + 1 == digits[2] && digits[2] + 1 == digits[3])
                || (digits[0] == digits[1] + 1 && digits[1] == digits[2] + 1 && digits[2] == digits[3] + 1);
            
            if is_sequential {
                return 0.6;
            }
        }
        
        0.0
    }

    /// Records a transaction for future analysis
    pub async fn record_transaction(
        &self,
        card_info: &CardInfo,
        amount: i64,
        success: bool,
        location: Option<String>,
    ) {
        let card_key = format!("card_{}", &card_info.pan[card_info.pan.len().saturating_sub(4)..]);
        let mut history = self.transaction_history.write().await;
        
        let record = TransactionRecord {
            amount,
            timestamp: chrono::Utc::now(),
            location,
            success,
        };
        
        history.entry(card_key.clone())
            .or_insert_with(Vec::new)
            .push(record);
        
        // Keep only last 100 transactions per card
        if let Some(transactions) = history.get_mut(&card_key) {
            if transactions.len() > 100 {
                transactions.remove(0);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FraudAnalysisResult {
    pub risk_score: f64,
    pub risk_level: RiskLevel,
    pub risk_factors: Vec<String>,
    pub should_block: bool,
    pub should_require_authentication: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl Default for AdvancedFraudDetector {
    fn default() -> Self {
        Self::new()
    }
}

