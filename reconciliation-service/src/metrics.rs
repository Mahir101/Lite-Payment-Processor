use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, IntGaugeVec, Opts, Registry,
};

lazy_static::lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Reconciliation metrics
    pub static ref RECONCILIATION_RUNS_TOTAL: IntCounter = IntCounter::new(
        "reconciliation_runs_total",
        "Total number of reconciliation runs"
    ).expect("metric can be created");
    
    pub static ref RECONCILIATION_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("reconciliation_duration_seconds", "Reconciliation run duration in seconds")
    ).expect("metric can be created");
    
    pub static ref ANOMALIES_DETECTED_TOTAL: IntCounter = IntCounter::new(
        "anomalies_detected_total",
        "Total number of anomalies detected"
    ).expect("metric can be created");
    
    pub static ref ANOMALIES_BY_TYPE: IntCounterVec = IntCounterVec::new(
        Opts::new("anomalies_by_type_total", "Total anomalies by type"),
        &["anomaly_type"]
    ).expect("metric can be created");
    
    pub static ref ANOMALIES_BY_SEVERITY: IntCounterVec = IntCounterVec::new(
        Opts::new("anomalies_by_severity_total", "Total anomalies by severity"),
        &["severity"]
    ).expect("metric can be created");

    // Event processing metrics
    pub static ref EVENTS_PROCESSED_TOTAL: IntCounter = IntCounter::new(
        "events_processed_total",
        "Total number of events processed"
    ).expect("metric can be created");
    
    pub static ref EVENTS_BY_TYPE: IntCounterVec = IntCounterVec::new(
        Opts::new("events_by_type_total", "Total events by type"),
        &["event_type"]
    ).expect("metric can be created");
    
    pub static ref EVENT_PROCESSING_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("event_processing_duration_seconds", "Event processing duration in seconds"),
        &["event_type"]
    ).expect("metric can be created");

    // Report generation metrics
    pub static ref REPORTS_GENERATED_TOTAL: IntCounter = IntCounter::new(
        "reports_generated_total",
        "Total number of reports generated"
    ).expect("metric can be created");
    
    pub static ref REPORT_GENERATION_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("report_generation_duration_seconds", "Report generation duration in seconds")
    ).expect("metric can be created");
    
    pub static ref CSV_REPORTS_DOWNLOADED: IntCounter = IntCounter::new(
        "csv_reports_downloaded_total",
        "Total number of CSV reports downloaded"
    ).expect("metric can be created");

    // Event replay metrics
    pub static ref EVENT_REPLAYS_TOTAL: IntCounter = IntCounter::new(
        "event_replays_total",
        "Total number of event replays"
    ).expect("metric can be created");
    
    pub static ref EVENT_REPLAY_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("event_replay_duration_seconds", "Event replay duration in seconds")
    ).expect("metric can be created");
    
    pub static ref EVENT_REPLAY_EVENTS_PROCESSED: IntCounter = IntCounter::new(
        "event_replay_events_processed_total",
        "Total events processed during replays"
    ).expect("metric can be created");
    
    pub static ref EVENT_REPLAY_ERRORS: IntCounter = IntCounter::new(
        "event_replay_errors_total",
        "Total errors during event replays"
    ).expect("metric can be created");

    // Daily summary metrics
    pub static ref DAILY_SUMMARIES_UPDATED: IntCounter = IntCounter::new(
        "daily_summaries_updated_total",
        "Total number of daily summary updates"
    ).expect("metric can be created");
    
    pub static ref DAILY_TRANSACTION_COUNT: Gauge = Gauge::new(
        "daily_transaction_count",
        "Current daily transaction count"
    ).expect("metric can be created");
    
    pub static ref DAILY_AMOUNT_TOTAL: Gauge = Gauge::new(
        "daily_amount_total",
        "Current daily amount total"
    ).expect("metric can be created");

    // HTTP metrics (same as payment processor)
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

    // Error metrics
    pub static ref ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("errors_total", "Total number of errors"),
        &["error_type", "service"]
    ).expect("metric can be created");
}

pub fn init_metrics() {
    REGISTRY.register(Box::new(RECONCILIATION_RUNS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(RECONCILIATION_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(ANOMALIES_DETECTED_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(ANOMALIES_BY_TYPE.clone())).unwrap();
    REGISTRY.register(Box::new(ANOMALIES_BY_SEVERITY.clone())).unwrap();
    REGISTRY.register(Box::new(EVENTS_PROCESSED_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(EVENTS_BY_TYPE.clone())).unwrap();
    REGISTRY.register(Box::new(EVENT_PROCESSING_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(REPORTS_GENERATED_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(REPORT_GENERATION_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(CSV_REPORTS_DOWNLOADED.clone())).unwrap();
    REGISTRY.register(Box::new(EVENT_REPLAYS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(EVENT_REPLAY_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(EVENT_REPLAY_EVENTS_PROCESSED.clone())).unwrap();
    REGISTRY.register(Box::new(EVENT_REPLAY_ERRORS.clone())).unwrap();
    REGISTRY.register(Box::new(DAILY_SUMMARIES_UPDATED.clone())).unwrap();
    REGISTRY.register(Box::new(DAILY_TRANSACTION_COUNT.clone())).unwrap();
    REGISTRY.register(Box::new(DAILY_AMOUNT_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(HTTP_REQUESTS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(HTTP_REQUEST_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(DATABASE_CONNECTIONS_ACTIVE.clone())).unwrap();
    REGISTRY.register(Box::new(DATABASE_QUERY_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(REDIS_OPERATIONS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(REDIS_OPERATION_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(ERRORS_TOTAL.clone())).unwrap();
}

pub fn get_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = REGISTRY.gather();
    encoder.encode_to_string(&metric_families).unwrap()
}

// Helper functions for reconciliation metrics
pub fn increment_reconciliation_run() {
    RECONCILIATION_RUNS_TOTAL.inc();
}

pub fn record_reconciliation_duration(duration: f64) {
    RECONCILIATION_DURATION.observe(duration);
}

pub fn increment_anomaly_detected(anomaly_type: &str, severity: &str) {
    ANOMALIES_DETECTED_TOTAL.inc();
    ANOMALIES_BY_TYPE.with_label_values(&[anomaly_type]).inc();
    ANOMALIES_BY_SEVERITY.with_label_values(&[severity]).inc();
}

pub fn increment_event_processed(event_type: &str) {
    EVENTS_PROCESSED_TOTAL.inc();
    EVENTS_BY_TYPE.with_label_values(&[event_type]).inc();
}

pub fn record_event_processing_duration(event_type: &str, duration: f64) {
    EVENT_PROCESSING_DURATION
        .with_label_values(&[event_type])
        .observe(duration);
}

pub fn increment_report_generated() {
    REPORTS_GENERATED_TOTAL.inc();
}

pub fn record_report_generation_duration(duration: f64) {
    REPORT_GENERATION_DURATION.observe(duration);
}

pub fn increment_csv_downloaded() {
    CSV_REPORTS_DOWNLOADED.inc();
}

pub fn increment_event_replay() {
    EVENT_REPLAYS_TOTAL.inc();
}

pub fn record_event_replay_duration(duration: f64) {
    EVENT_REPLAY_DURATION.observe(duration);
}

pub fn increment_replay_events_processed(count: i64) {
    EVENT_REPLAY_EVENTS_PROCESSED.inc_by(count as u64);
}

pub fn increment_replay_errors() {
    EVENT_REPLAY_ERRORS.inc();
}

pub fn increment_daily_summary_updated() {
    DAILY_SUMMARIES_UPDATED.inc();
}

pub fn update_daily_transaction_count(count: i64) {
    DAILY_TRANSACTION_COUNT.set(count as f64);
}

pub fn update_daily_amount_total(amount: i64) {
    DAILY_AMOUNT_TOTAL.set(amount as f64);
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
