use prometheus::{
    Counter, HistogramOpts, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, IntGaugeVec, Opts, Registry,
};

lazy_static::lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Transaction metrics
    pub static ref TRANSACTION_TOTAL: IntCounter = IntCounter::new(
        "transactions_total",
        "Total number of transactions processed"
    ).expect("metric can be created");
    
    pub static ref TRANSACTION_CREATED: IntCounter = IntCounter::new(
        "transactions_created_total",
        "Total number of transactions created"
    ).expect("metric can be created");
    
    pub static ref TRANSACTION_COMMITTED: IntCounter = IntCounter::new(
        "transactions_committed_total",
        "Total number of transactions committed"
    ).expect("metric can be created");
    
    pub static ref TRANSACTION_FAILED: IntCounter = IntCounter::new(
        "transactions_failed_total",
        "Total number of transactions failed"
    ).expect("metric can be created");
    
    pub static ref TRANSACTION_CANCELLED: IntCounter = IntCounter::new(
        "transactions_cancelled_total",
        "Total number of transactions cancelled"
    ).expect("metric can be created");

    // Transaction amount metrics
    pub static ref TRANSACTION_AMOUNT_TOTAL: Counter = Counter::new(
        "transaction_amount_total",
        "Total amount of all transactions in cents"
    ).expect("metric can be created");

    // Request metrics
    pub static ref HTTP_REQUESTS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("http_requests_total", "Total number of HTTP requests"),
        &["method", "endpoint", "status"]
    ).expect("metric can be created");
    
    pub static ref HTTP_REQUEST_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("http_request_duration_seconds", "HTTP request duration in seconds"),
        &["method", "endpoint"]
    ).expect("metric can be created");

    // Database metrics
    pub static ref DATABASE_CONNECTIONS_ACTIVE: IntGauge = IntGauge::new(
        "database_connections_active",
        "Number of active database connections"
    ).expect("metric can be created");
    
    pub static ref DATABASE_QUERY_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("database_query_duration_seconds", "Database query duration in seconds"),
        &["query_type"]
    ).expect("metric can be created");

    // Redis metrics
    pub static ref REDIS_OPERATIONS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("redis_operations_total", "Total number of Redis operations"),
        &["operation", "status"]
    ).expect("metric can be created");
    
    pub static ref REDIS_OPERATION_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("redis_operation_duration_seconds", "Redis operation duration in seconds"),
        &["operation"]
    ).expect("metric can be created");

    // Business metrics
    pub static ref TRANSACTIONS_BY_CURRENCY: IntCounterVec = IntCounterVec::new(
        Opts::new("transactions_by_currency_total", "Total transactions by currency"),
        &["currency"]
    ).expect("metric can be created");
    
    pub static ref TRANSACTIONS_BY_STATE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("transactions_by_state", "Current number of transactions by state"),
        &["state"]
    ).expect("metric can be created");

    // Error metrics
    pub static ref ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("errors_total", "Total number of errors"),
        &["error_type", "service"]
    ).expect("metric can be created");

    // Outbox metrics
    pub static ref OUTBOX_EVENTS_TOTAL: IntCounter = IntCounter::new(
        "outbox_events_total",
        "Total number of outbox events processed"
    ).expect("metric can be created");
    
    pub static ref OUTBOX_EVENTS_PENDING: IntGauge = IntGauge::new(
        "outbox_events_pending",
        "Number of pending outbox events"
    ).expect("metric can be created");
    
    pub static ref OUTBOX_EVENTS_FAILED: IntCounter = IntCounter::new(
        "outbox_events_failed_total",
        "Total number of failed outbox events"
    ).expect("metric can be created");
}

pub fn init_metrics() {
    REGISTRY.register(Box::new(TRANSACTION_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTION_CREATED.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTION_COMMITTED.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTION_FAILED.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTION_CANCELLED.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTION_AMOUNT_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(HTTP_REQUESTS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(HTTP_REQUEST_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(DATABASE_CONNECTIONS_ACTIVE.clone())).unwrap();
    REGISTRY.register(Box::new(DATABASE_QUERY_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(REDIS_OPERATIONS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(REDIS_OPERATION_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTIONS_BY_CURRENCY.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTIONS_BY_STATE.clone())).unwrap();
    REGISTRY.register(Box::new(ERRORS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(OUTBOX_EVENTS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(OUTBOX_EVENTS_PENDING.clone())).unwrap();
    REGISTRY.register(Box::new(OUTBOX_EVENTS_FAILED.clone())).unwrap();
}

pub fn get_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = REGISTRY.gather();
    encoder.encode_to_string(&metric_families).unwrap()
}

// Helper functions for common metric updates
pub fn increment_transaction_created(currency: &str) {
    TRANSACTION_CREATED.inc();
    TRANSACTION_TOTAL.inc();
    TRANSACTIONS_BY_CURRENCY.with_label_values(&[currency]).inc();
}

pub fn increment_transaction_committed() {
    TRANSACTION_COMMITTED.inc();
}

pub fn increment_transaction_failed() {
    TRANSACTION_FAILED.inc();
}

pub fn increment_transaction_cancelled() {
    TRANSACTION_CANCELLED.inc();
}

pub fn add_transaction_amount(amount: i64) {
    TRANSACTION_AMOUNT_TOTAL.inc_by(amount as f64);
}

pub fn update_transaction_state_count(state: &str, count: i64) {
    TRANSACTIONS_BY_STATE.with_label_values(&[state]).set(count);
}

pub fn increment_http_request(method: &str, endpoint: &str, status: u16) {
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, endpoint, &status.to_string()])
        .inc();
}

pub fn record_http_duration(method: &str, endpoint: &str, duration: f64) {
    HTTP_REQUEST_DURATION
        .with_label_values(&[method, endpoint])
        .observe(duration);
}

pub fn increment_database_query(_query_type: &str) {
    // This would be called for each database query
}

pub fn record_database_duration(query_type: &str, duration: f64) {
    DATABASE_QUERY_DURATION
        .with_label_values(&[query_type])
        .observe(duration);
}

pub fn increment_redis_operation(operation: &str, status: &str) {
    REDIS_OPERATIONS_TOTAL
        .with_label_values(&[operation, status])
        .inc();
}

pub fn record_redis_duration(operation: &str, duration: f64) {
    REDIS_OPERATION_DURATION
        .with_label_values(&[operation])
        .observe(duration);
}

pub fn increment_error(error_type: &str, service: &str) {
    ERRORS_TOTAL
        .with_label_values(&[error_type, service])
        .inc();
}

pub fn increment_outbox_event() {
    OUTBOX_EVENTS_TOTAL.inc();
}

pub fn update_outbox_pending(count: i64) {
    OUTBOX_EVENTS_PENDING.set(count);
}

pub fn increment_outbox_failed() {
    OUTBOX_EVENTS_FAILED.inc();
}
