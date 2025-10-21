-- PostgreSQL Monitoring Queries for Lite Payment Processor
-- These queries help monitor database activity and performance

-- ==============================================
-- PAYMENT PROCESSOR DATABASE MONITORING QUERIES
-- ==============================================

-- 1. Real-time Transaction Monitoring
-- Monitor recent transaction updates and state changes
SELECT 
    id,
    external_id,
    state,
    amount,
    currency,
    from_account,
    to_account,
    created_at,
    updated_at,
    EXTRACT(EPOCH FROM (updated_at - created_at)) as processing_time_seconds,
    CASE 
        WHEN updated_at > NOW() - INTERVAL '1 minute' THEN 'Very Recent'
        WHEN updated_at > NOW() - INTERVAL '5 minutes' THEN 'Recent'
        WHEN updated_at > NOW() - INTERVAL '15 minutes' THEN 'Moderate'
        ELSE 'Old'
    END as recency
FROM transactions 
WHERE updated_at > NOW() - INTERVAL '1 hour'
ORDER BY updated_at DESC 
LIMIT 20;

-- 2. Transaction State Distribution
-- Show current distribution of transaction states
SELECT 
    state,
    COUNT(*) as count,
    ROUND(COUNT(*) * 100.0 / SUM(COUNT(*)) OVER(), 2) as percentage,
    AVG(EXTRACT(EPOCH FROM (updated_at - created_at))) as avg_processing_time_seconds,
    MIN(created_at) as oldest_transaction,
    MAX(updated_at) as latest_update
FROM transactions 
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY state
ORDER BY count DESC;

-- 3. Transaction Events Audit Trail
-- Monitor all transaction events for audit purposes
SELECT 
    te.id as event_id,
    te.transaction_id,
    t.external_id,
    te.event_type,
    te.created_at as event_time,
    t.state as current_state,
    t.amount,
    t.currency
FROM transaction_events te
JOIN transactions t ON te.transaction_id = t.id
WHERE te.created_at > NOW() - INTERVAL '1 hour'
ORDER BY te.created_at DESC;

-- 4. Outbox Events Monitoring
-- Monitor event publishing reliability
SELECT 
    id,
    aggregate_id,
    aggregate_type,
    event_type,
    status,
    created_at,
    processed_at,
    retry_count,
    CASE 
        WHEN processed_at IS NULL AND created_at < NOW() - INTERVAL '5 minutes' THEN 'Stuck'
        WHEN status = 'FAILED' THEN 'Failed'
        WHEN status = 'PENDING' THEN 'Pending'
        WHEN status = 'COMPLETED' THEN 'Completed'
        ELSE 'Processing'
    END as health_status
FROM outbox_events 
WHERE created_at > NOW() - INTERVAL '1 hour'
ORDER BY created_at DESC;

-- 5. Database Performance Monitoring
-- Monitor active connections and queries
SELECT 
    pid,
    usename,
    application_name,
    client_addr,
    state,
    query_start,
    state_change,
    LEFT(query, 100) as query_preview,
    EXTRACT(EPOCH FROM (NOW() - query_start)) as query_duration_seconds
FROM pg_stat_activity 
WHERE datname = 'payment_processor'
AND state != 'idle'
ORDER BY query_start DESC;

-- 6. Transaction Volume by Time
-- Monitor transaction volume over time
SELECT 
    DATE_TRUNC('minute', created_at) as minute,
    COUNT(*) as transaction_count,
    COUNT(CASE WHEN state = 'COMMITTED' THEN 1 END) as committed_count,
    COUNT(CASE WHEN state = 'FAILED' THEN 1 END) as failed_count,
    SUM(amount) as total_amount,
    AVG(amount) as avg_amount
FROM transactions 
WHERE created_at > NOW() - INTERVAL '1 hour'
GROUP BY DATE_TRUNC('minute', created_at)
ORDER BY minute DESC;

-- 7. Account Activity Monitoring
-- Monitor account activity patterns
SELECT 
    from_account,
    COUNT(*) as outgoing_transactions,
    SUM(amount) as outgoing_amount,
    COUNT(DISTINCT to_account) as unique_destinations
FROM transactions 
WHERE created_at > NOW() - INTERVAL '1 hour'
GROUP BY from_account
ORDER BY outgoing_transactions DESC
LIMIT 10;

-- 8. Idempotency Key Monitoring
-- Monitor idempotency key usage and cleanup
SELECT 
    COUNT(*) as total_keys,
    COUNT(CASE WHEN expires_at > NOW() THEN 1 END) as active_keys,
    COUNT(CASE WHEN expires_at <= NOW() THEN 1 END) as expired_keys,
    MIN(created_at) as oldest_key,
    MAX(expires_at) as latest_expiry
FROM idempotency_keys;

-- ==============================================
-- RECONCILIATION DATABASE MONITORING QUERIES
-- ==============================================

-- 9. Event Processing Monitoring
-- Monitor event processing in reconciliation service
SELECT 
    event_id,
    event_type,
    transaction_id,
    processed_at,
    created_at,
    EXTRACT(EPOCH FROM (processed_at - created_at)) as processing_delay_seconds
FROM event_ledger 
WHERE processed_at > NOW() - INTERVAL '1 hour'
ORDER BY processed_at DESC;

-- 10. Daily Summary Updates
-- Monitor daily summary calculations
SELECT 
    date,
    total_transactions,
    total_amount,
    committed_transactions,
    committed_amount,
    failed_transactions,
    failed_amount,
    anomalies_count,
    last_reconciliation_at,
    updated_at,
    EXTRACT(EPOCH FROM (updated_at - created_at)) as summary_age_seconds
FROM daily_summaries 
WHERE updated_at > NOW() - INTERVAL '1 hour'
ORDER BY updated_at DESC;

-- 11. Anomaly Detection Monitoring
-- Monitor detected anomalies and their resolution
SELECT 
    anomaly_id,
    anomaly_type,
    severity,
    status,
    transaction_id,
    detected_at,
    resolved_at,
    resolved_by,
    CASE 
        WHEN resolved_at IS NULL AND detected_at < NOW() - INTERVAL '1 hour' THEN 'Overdue'
        WHEN resolved_at IS NULL THEN 'Open'
        WHEN resolved_at IS NOT NULL THEN 'Resolved'
    END as resolution_status
FROM anomalies 
WHERE detected_at > NOW() - INTERVAL '24 hours'
ORDER BY detected_at DESC;

-- 12. Reconciliation Run Monitoring
-- Monitor reconciliation execution
SELECT 
    id,
    run_date,
    start_time,
    end_time,
    status,
    transactions_processed,
    anomalies_found,
    EXTRACT(EPOCH FROM (COALESCE(end_time, NOW()) - start_time)) as duration_seconds,
    error_message
FROM reconciliation_runs 
WHERE start_time > NOW() - INTERVAL '24 hours'
ORDER BY start_time DESC;

-- 13. Event Replay Monitoring
-- Monitor event replay operations
SELECT 
    id,
    replay_id,
    status,
    started_at,
    completed_at,
    events_processed,
    events_total,
    errors_count,
    ROUND(events_processed * 100.0 / NULLIF(events_total, 0), 2) as progress_percentage,
    EXTRACT(EPOCH FROM (COALESCE(completed_at, NOW()) - started_at)) as duration_seconds
FROM event_replays 
WHERE started_at > NOW() - INTERVAL '24 hours'
ORDER BY started_at DESC;

-- ==============================================
-- PERFORMANCE AND HEALTH MONITORING QUERIES
-- ==============================================

-- 14. Database Size and Growth
-- Monitor database and table sizes
SELECT 
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as total_size,
    pg_size_pretty(pg_relation_size(schemaname||'.'||tablename)) as table_size,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename) - pg_relation_size(schemaname||'.'||tablename)) as index_size
FROM pg_tables 
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;

-- 15. Index Usage Statistics
-- Monitor index usage for optimization
SELECT 
    schemaname,
    tablename,
    indexname,
    idx_tup_read,
    idx_tup_fetch,
    idx_scan,
    CASE 
        WHEN idx_scan = 0 THEN 'Unused'
        WHEN idx_scan < 100 THEN 'Low Usage'
        WHEN idx_scan < 1000 THEN 'Medium Usage'
        ELSE 'High Usage'
    END as usage_level
FROM pg_stat_user_indexes 
ORDER BY idx_scan DESC;

-- 16. Slow Query Identification
-- Identify potentially slow queries (requires pg_stat_statements extension)
SELECT 
    query,
    calls,
    total_time,
    mean_time,
    rows,
    ROUND(100.0 * shared_blks_hit / NULLIF(shared_blks_hit + shared_blks_read, 0), 2) as hit_percent
FROM pg_stat_statements 
ORDER BY mean_time DESC 
LIMIT 10;

-- 17. Connection Pool Monitoring
-- Monitor database connections
SELECT 
    datname,
    COUNT(*) as total_connections,
    COUNT(CASE WHEN state = 'active' THEN 1 END) as active_connections,
    COUNT(CASE WHEN state = 'idle' THEN 1 END) as idle_connections,
    COUNT(CASE WHEN state = 'idle in transaction' THEN 1 END) as idle_in_transaction,
    MAX(EXTRACT(EPOCH FROM (NOW() - state_change))) as max_idle_time_seconds
FROM pg_stat_activity 
WHERE datname IN ('payment_processor', 'reconciliation')
GROUP BY datname;

-- 18. Lock Monitoring
-- Monitor database locks
SELECT 
    l.locktype,
    l.database,
    l.relation::regclass,
    l.page,
    l.tuple,
    l.virtualxid,
    l.transactionid,
    l.classid,
    l.objid,
    l.objsubid,
    l.virtualtransaction,
    l.pid,
    l.mode,
    l.granted,
    a.usename,
    a.query,
    a.query_start,
    a.state
FROM pg_locks l
LEFT JOIN pg_stat_activity a ON l.pid = a.pid
WHERE NOT l.granted
ORDER BY l.pid;

-- ==============================================
-- BUSINESS INTELLIGENCE QUERIES
-- ==============================================

-- 19. Transaction Success Rate
-- Calculate transaction success rates
SELECT 
    DATE_TRUNC('hour', created_at) as hour,
    COUNT(*) as total_transactions,
    COUNT(CASE WHEN state = 'COMMITTED' THEN 1 END) as successful_transactions,
    COUNT(CASE WHEN state = 'FAILED' THEN 1 END) as failed_transactions,
    ROUND(COUNT(CASE WHEN state = 'COMMITTED' THEN 1 END) * 100.0 / COUNT(*), 2) as success_rate_percent
FROM transactions 
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY DATE_TRUNC('hour', created_at)
ORDER BY hour DESC;

-- 20. Currency Distribution
-- Monitor transaction distribution by currency
SELECT 
    currency,
    COUNT(*) as transaction_count,
    SUM(amount) as total_amount,
    AVG(amount) as avg_amount,
    MIN(amount) as min_amount,
    MAX(amount) as max_amount,
    ROUND(COUNT(*) * 100.0 / SUM(COUNT(*)) OVER(), 2) as percentage_of_transactions
FROM transactions 
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY currency
ORDER BY transaction_count DESC;

-- 21. Peak Hours Analysis
-- Identify peak transaction hours
SELECT 
    EXTRACT(hour FROM created_at) as hour_of_day,
    COUNT(*) as transaction_count,
    SUM(amount) as total_amount,
    AVG(amount) as avg_amount
FROM transactions 
WHERE created_at > NOW() - INTERVAL '7 days'
GROUP BY EXTRACT(hour FROM created_at)
ORDER BY transaction_count DESC;

-- 22. Account Balance Impact
-- Monitor account balance changes (if implemented)
SELECT 
    account_number,
    COUNT(*) as transaction_count,
    SUM(CASE WHEN from_account = account_number THEN -amount ELSE amount END) as net_change,
    SUM(amount) as total_volume
FROM (
    SELECT from_account as account_number, amount FROM transactions WHERE created_at > NOW() - INTERVAL '1 hour'
    UNION ALL
    SELECT to_account as account_number, amount FROM transactions WHERE created_at > NOW() - INTERVAL '1 hour'
) account_transactions
GROUP BY account_number
ORDER BY ABS(SUM(CASE WHEN from_account = account_number THEN -amount ELSE amount END)) DESC
LIMIT 10;

