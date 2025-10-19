use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("Invalid transaction format: {0}")]
    InvalidFormat(String),
    #[error("Transaction not found: {0}")]
    TransactionNotFound(Uuid),
    #[error("Invalid state transition: {0} -> {1}")]
    InvalidStateTransition(TransactionState, TransactionState),
    #[error("Duplicate transaction: {0}")]
    DuplicateTransaction(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Redis error: {0}")]
    RedisError(String),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Invalid card: {0}")]
    InvalidCard(String),
    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),
    #[error("User not found: {0}")]
    UserNotFound(String),
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    #[error("Invalid account: {0}")]
    InvalidAccount(String),
    #[error("Card validation failed: {0}")]
    CardValidationFailed(String),
    #[error("Fraud detection: {0}")]
    FraudDetected(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionState {
    Pending,
    Committed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TransactionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionState::Pending => write!(f, "PENDING"),
            TransactionState::Committed => write!(f, "COMMITTED"),
            TransactionState::Failed => write!(f, "FAILED"),
            TransactionState::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

// Card Information Structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInfo {
    pub pan: String, // Primary Account Number (card number)
    pub expiry_month: u8,
    pub expiry_year: u16,
    pub cvv: String,
    pub cardholder_name: String,
    pub billing_address: BillingAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingAddress {
    pub line1: String,
    pub city: String,
    pub postal_code: String,
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub phone: Option<String>,
    pub device_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub is_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub user_id: Uuid,
    pub account_number: String,
    pub balance: i64, // Balance in cents
    pub currency: String,
    pub account_type: AccountType,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountType {
    Checking,
    Savings,
    Credit,
    Debit,
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountType::Checking => write!(f, "CHECKING"),
            AccountType::Savings => write!(f, "SAVINGS"),
            AccountType::Credit => write!(f, "CREDIT"),
            AccountType::Debit => write!(f, "DEBIT"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub from_account: String,
    pub to_account: String,
    pub amount: i64,
    pub currency: String,
    pub description: Option<String>,
    pub card_info: Option<CardInfo>,
    pub user_info: Option<UserInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub external_id: String,
    pub amount: i64, // Amount in cents to avoid floating point issues
    pub currency: String,
    pub from_account: String,
    pub to_account: String,
    pub description: Option<String>,
    pub state: TransactionState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub external_id: String,
    pub amount: i64,
    pub currency: String,
    pub from_account: String,
    pub to_account: String,
    pub description: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iso8583Request {
    pub message_type: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEvent {
    pub event_id: Uuid,
    pub transaction_id: Uuid,
    pub event_type: TransactionEventType,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionEventType {
    Created,
    StateChanged { from: TransactionState, to: TransactionState },
    Failed { reason: String },
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationReport {
    pub report_id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_transactions: i64,
    pub total_amount: i64,
    pub anomalies: Vec<Anomaly>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub anomaly_id: Uuid,
    pub transaction_id: Option<Uuid>,
    pub anomaly_type: AnomalyType,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub severity: AnomalySeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    MissingTransaction,
    AmountMismatch,
    StateMismatch,
    DuplicateTransaction,
    OrphanedEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub service: String,
    pub status: HealthStatus,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub dependencies: HashMap<String, DependencyHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyHealth {
    pub status: HealthStatus,
    pub response_time_ms: Option<u64>,
    pub last_check: DateTime<Utc>,
}

// JWT Claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub aud: String,
}

// API Response types
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: Utc::now(),
        }
    }
}



