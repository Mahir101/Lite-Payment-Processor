-- Reconciliation Service Database Schema
-- This schema is optimized for reconciliation and reporting

-- Event ledger table - stores all events from payment processor
CREATE TABLE event_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id VARCHAR(255) NOT NULL UNIQUE,
    event_type VARCHAR(50) NOT NULL,
    transaction_id UUID NOT NULL,
    event_data JSONB NOT NULL DEFAULT '{}'::jsonb,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for event lookups
CREATE INDEX idx_event_ledger_event_id ON event_ledger(event_id);
CREATE INDEX idx_event_ledger_transaction_id ON event_ledger(transaction_id);
CREATE INDEX idx_event_ledger_event_type ON event_ledger(event_type);
CREATE INDEX idx_event_ledger_processed_at ON event_ledger(processed_at);

-- Daily reconciliation summaries
CREATE TABLE daily_summaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    date DATE NOT NULL UNIQUE,
    total_transactions INTEGER NOT NULL DEFAULT 0,
    total_amount BIGINT NOT NULL DEFAULT 0,
    committed_transactions INTEGER NOT NULL DEFAULT 0,
    committed_amount BIGINT NOT NULL DEFAULT 0,
    failed_transactions INTEGER NOT NULL DEFAULT 0,
    failed_amount BIGINT NOT NULL DEFAULT 0,
    anomalies_count INTEGER NOT NULL DEFAULT 0,
    last_reconciliation_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for date-based queries
CREATE INDEX idx_daily_summaries_date ON daily_summaries(date);

-- Anomalies table - stores detected discrepancies
CREATE TABLE anomalies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    anomaly_type VARCHAR(50) NOT NULL,
    description TEXT NOT NULL,
    transaction_id UUID,
    expected_value JSONB,
    actual_value JSONB,
    severity VARCHAR(20) NOT NULL CHECK (severity IN ('LOW', 'MEDIUM', 'HIGH', 'CRITICAL')),
    status VARCHAR(20) NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN', 'INVESTIGATING', 'RESOLVED', 'IGNORED')),
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    resolved_by VARCHAR(255),
    resolution_notes TEXT
);

-- Indexes for anomaly queries
CREATE INDEX idx_anomalies_type ON anomalies(anomaly_type);
CREATE INDEX idx_anomalies_severity ON anomalies(severity);
CREATE INDEX idx_anomalies_status ON anomalies(status);
CREATE INDEX idx_anomalies_detected_at ON anomalies(detected_at);
CREATE INDEX idx_anomalies_transaction_id ON anomalies(transaction_id);

-- Reconciliation runs table - tracks reconciliation execution
CREATE TABLE reconciliation_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_date DATE NOT NULL,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    status VARCHAR(20) NOT NULL CHECK (status IN ('RUNNING', 'COMPLETED', 'FAILED')),
    transactions_processed INTEGER NOT NULL DEFAULT 0,
    anomalies_found INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for reconciliation run queries
CREATE INDEX idx_reconciliation_runs_date ON reconciliation_runs(run_date);
CREATE INDEX idx_reconciliation_runs_status ON reconciliation_runs(status);

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Trigger to automatically update updated_at for daily_summaries
CREATE TRIGGER update_daily_summaries_updated_at 
    BEFORE UPDATE ON daily_summaries 
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();

-- Event Replay tracking table
CREATE TABLE event_replays (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    replay_id UUID NOT NULL UNIQUE,
    status VARCHAR(20) NOT NULL CHECK (status IN ('RUNNING', 'COMPLETED', 'FAILED', 'CANCELLED')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    events_processed INTEGER NOT NULL DEFAULT 0,
    events_total INTEGER NOT NULL DEFAULT 0,
    errors_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for event replays
CREATE INDEX idx_event_replays_status ON event_replays(status);
CREATE INDEX idx_event_replays_started_at ON event_replays(started_at);