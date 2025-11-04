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

// ========== STRIPE-LIKE FEATURES ==========

// Refund types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RefundStatus {
    Pending,
    Succeeded,
    Failed,
    Cancelled,
}

impl std::fmt::Display for RefundStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefundStatus::Pending => write!(f, "PENDING"),
            RefundStatus::Succeeded => write!(f, "SUCCEEDED"),
            RefundStatus::Failed => write!(f, "FAILED"),
            RefundStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RefundReason {
    RequestedByCustomer,
    Duplicate,
    Fraudulent,
    Other,
}

impl std::fmt::Display for RefundReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefundReason::RequestedByCustomer => write!(f, "requested_by_customer"),
            RefundReason::Duplicate => write!(f, "duplicate"),
            RefundReason::Fraudulent => write!(f, "fraudulent"),
            RefundReason::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refund {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub amount: i64,
    pub currency: String,
    pub reason: Option<RefundReason>,
    pub status: RefundStatus,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Customer types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: Uuid,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Payment Method types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PaymentMethodType {
    Card,
    Ach,
    BankAccount,
    Paypal,
    ApplePay,
    GooglePay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CardBrand {
    Visa,
    Mastercard,
    Amex,
    Discover,
    Jcb,
    DinersClub,
    UnionPay,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub id: Uuid,
    pub customer_id: Option<Uuid>,
    pub r#type: PaymentMethodType,
    pub card_token: String, // Tokenized card
    pub card_brand: Option<CardBrand>,
    pub card_last4: Option<String>,
    pub card_exp_month: Option<u8>,
    pub card_exp_year: Option<u16>,
    pub cardholder_name: Option<String>,
    pub billing_address: Option<BillingAddress>,
    pub is_default: bool,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Payment Intent types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PaymentIntentStatus {
    RequiresPaymentMethod,
    RequiresConfirmation,
    RequiresAction,
    Processing,
    RequiresCapture,
    Canceled,
    Succeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfirmationMethod {
    Automatic,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentIntent {
    pub id: Uuid,
    pub customer_id: Option<Uuid>,
    pub payment_method_id: Option<Uuid>,
    pub amount: i64,
    pub currency: String,
    pub status: PaymentIntentStatus,
    pub confirmation_method: ConfirmationMethod,
    pub client_secret: Option<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Subscription types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SubscriptionStatus {
    Incomplete,
    IncompleteExpired,
    Trialing,
    Active,
    PastDue,
    Canceled,
    Unpaid,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub status: SubscriptionStatus,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<DateTime<Utc>>,
    pub trial_start: Option<DateTime<Utc>>,
    pub trial_end: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Product and Price types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecurringInterval {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub id: Uuid,
    pub product_id: Uuid,
    pub amount: i64,
    pub currency: String,
    pub recurring_interval: Option<RecurringInterval>,
    pub recurring_interval_count: Option<u32>,
    pub active: bool,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub price_id: Option<Uuid>,
    pub quantity: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Dispute types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DisputeStatus {
    WarningNeedsResponse,
    WarningUnderReview,
    WarningClosed,
    NeedsResponse,
    UnderReview,
    ChargeRefunded,
    Won,
    Lost,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DisputeReason {
    BankCannotProcess,
    CheckReturned,
    CreditNotProcessed,
    CustomerInitiated,
    DebitNotAuthorized,
    Duplicate,
    Fraudulent,
    General,
    IncorrectAccountDetails,
    InsufficientFunds,
    ProductNotReceived,
    ProductUnacceptable,
    SubscriptionCanceled,
    Unrecognized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub amount: i64,
    pub currency: String,
    pub status: DisputeStatus,
    pub reason: Option<DisputeReason>,
    pub evidence_due_by: Option<DateTime<Utc>>,
    pub evidence_submitted_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeEvidence {
    pub id: Uuid,
    pub dispute_id: Uuid,
    pub evidence_type: String,
    pub evidence_data: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// Invoice types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InvoiceStatus {
    Draft,
    Open,
    Paid,
    Uncollectible,
    Void,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub subscription_id: Option<Uuid>,
    pub status: InvoiceStatus,
    pub amount_due: i64,
    pub amount_paid: i64,
    pub currency: String,
    pub due_date: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub invoice_pdf_url: Option<String>,
    pub hosted_invoice_url: Option<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceLineItem {
    pub id: Uuid,
    pub invoice_id: Uuid,
    pub description: Option<String>,
    pub amount: i64,
    pub quantity: u32,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

// Payout types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PayoutStatus {
    Pending,
    InTransit,
    Paid,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PayoutMethod {
    BankAccount,
    Card,
    Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payout {
    pub id: Uuid,
    pub account_id: Uuid,
    pub amount: i64,
    pub currency: String,
    pub status: PayoutStatus,
    pub payout_method: PayoutMethod,
    pub arrival_date: Option<DateTime<Utc>>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Webhook types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WebhookEventStatus {
    Pending,
    Delivered,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    pub id: Uuid,
    pub url: String,
    pub secret_key: String,
    pub events: Vec<String>,
    pub active: bool,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub status: WebhookEventStatus,
    pub attempts: u32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub response_status: Option<u16>,
    pub response_body: Option<String>,
    pub created_at: DateTime<Utc>,
}

// Connect/Marketplace types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectAccountType {
    Express,
    Standard,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectAccount {
    pub id: Uuid,
    pub email: Option<String>,
    pub country: String,
    pub r#type: ConnectAccountType,
    pub charges_enabled: bool,
    pub payouts_enabled: bool,
    pub details_submitted: bool,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransferStatus {
    Pending,
    Paid,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transfer {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub destination_account_id: Uuid,
    pub amount: i64,
    pub currency: String,
    pub status: TransferStatus,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Tax types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxRate {
    pub id: Uuid,
    pub display_name: String,
    pub percentage: f64,
    pub inclusive: bool,
    pub country: Option<String>,
    pub jurisdiction: Option<String>,
    pub active: bool,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Exchange Rate types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRate {
    pub id: Uuid,
    pub base_currency: String,
    pub target_currency: String,
    pub rate: f64,
    pub effective_date: chrono::NaiveDate,
    pub created_at: DateTime<Utc>,
}

// Error extensions
impl PaymentError {
    pub fn refund_error(msg: String) -> Self {
        PaymentError::InvalidFormat(format!("Refund error: {}", msg))
    }
    
    pub fn subscription_error(msg: String) -> Self {
        PaymentError::InvalidFormat(format!("Subscription error: {}", msg))
    }
    
    pub fn dispute_error(msg: String) -> Self {
        PaymentError::InvalidFormat(format!("Dispute error: {}", msg))
    }
}



