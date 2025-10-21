# PostgreSQL Database Monitoring Script for Lite Payment Processor
# This script monitors real-time database updates and activity

param(
    [string]$Database = "payment",
    [int]$IntervalSeconds = 5,
    [switch]$ShowQueries = $false,
    [switch]$ShowMetrics = $false
)

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "PostgreSQL Database Monitor" -ForegroundColor Cyan
Write-Host "Database: $Database" -ForegroundColor Yellow
Write-Host "Interval: $IntervalSeconds seconds" -ForegroundColor Yellow
Write-Host "Press Ctrl+C to stop" -ForegroundColor Red
Write-Host "===============================================" -ForegroundColor Cyan

function Get-DatabaseStats {
    param([string]$db)
    
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "`n[$timestamp] Database Activity:" -ForegroundColor Green
    
    if ($db -eq "payment") {
        # Payment Processor Database Monitoring
        Write-Host "`n--- Payment Processor Database ---" -ForegroundColor Magenta
        
        # Recent transactions
        $transactionStats = docker exec lite-payment-processor-payment-db-1 psql -U postgres -d payment_processor -t -c "
        SELECT 
            COUNT(*) as recent_transactions,
            COUNT(CASE WHEN state = 'PENDING' THEN 1 END) as pending,
            COUNT(CASE WHEN state = 'COMMITTED' THEN 1 END) as committed,
            COUNT(CASE WHEN state = 'FAILED' THEN 1 END) as failed,
            COUNT(CASE WHEN state = 'CANCELLED' THEN 1 END) as cancelled
        FROM transactions 
        WHERE updated_at > NOW() - INTERVAL '1 minute';"
        
        Write-Host "Recent Transactions (last minute):" -ForegroundColor White
        Write-Host "  Total: $($transactionStats.Split('|')[0].Trim())" -ForegroundColor White
        Write-Host "  Pending: $($transactionStats.Split('|')[1].Trim())" -ForegroundColor Yellow
        Write-Host "  Committed: $($transactionStats.Split('|')[2].Trim())" -ForegroundColor Green
        Write-Host "  Failed: $($transactionStats.Split('|')[3].Trim())" -ForegroundColor Red
        Write-Host "  Cancelled: $($transactionStats.Split('|')[4].Trim())" -ForegroundColor Gray
        
        # Outbox events
        $outboxStats = docker exec lite-payment-processor-payment-db-1 psql -U postgres -d payment_processor -t -c "
        SELECT 
            COUNT(CASE WHEN status = 'PENDING' THEN 1 END) as pending,
            COUNT(CASE WHEN status = 'PROCESSING' THEN 1 END) as processing,
            COUNT(CASE WHEN status = 'COMPLETED' THEN 1 END) as completed,
            COUNT(CASE WHEN status = 'FAILED' THEN 1 END) as failed
        FROM outbox_events 
        WHERE created_at > NOW() - INTERVAL '1 minute';"
        
        Write-Host "`nOutbox Events (last minute):" -ForegroundColor White
        Write-Host "  Pending: $($outboxStats.Split('|')[0].Trim())" -ForegroundColor Yellow
        Write-Host "  Processing: $($outboxStats.Split('|')[1].Trim())" -ForegroundColor Blue
        Write-Host "  Completed: $($outboxStats.Split('|')[2].Trim())" -ForegroundColor Green
        Write-Host "  Failed: $($outboxStats.Split('|')[3].Trim())" -ForegroundColor Red
        
        # Transaction events
        $eventStats = docker exec lite-payment-processor-payment-db-1 psql -U postgres -d payment_processor -t -c "
        SELECT COUNT(*) as recent_events
        FROM transaction_events 
        WHERE created_at > NOW() - INTERVAL '1 minute';"
        
        Write-Host "`nTransaction Events (last minute): $($eventStats.Trim())" -ForegroundColor White
        
        # Database connections
        $connectionStats = docker exec lite-payment-processor-payment-db-1 psql -U postgres -d payment_processor -t -c "
        SELECT 
            COUNT(*) as total_connections,
            COUNT(CASE WHEN state = 'active' THEN 1 END) as active_connections
        FROM pg_stat_activity 
        WHERE datname = 'payment_processor';"
        
        Write-Host "`nDatabase Connections:" -ForegroundColor White
        Write-Host "  Total: $($connectionStats.Split('|')[0].Trim())" -ForegroundColor White
        Write-Host "  Active: $($connectionStats.Split('|')[1].Trim())" -ForegroundColor Green
        
    } else {
        # Reconciliation Database Monitoring
        Write-Host "`n--- Reconciliation Database ---" -ForegroundColor Magenta
        
        # Event processing
        $eventStats = docker exec lite-payment-processor-reconciliation-db-1 psql -U postgres -d reconciliation -t -c "
        SELECT 
            COUNT(*) as events_processed,
            COUNT(DISTINCT transaction_id) as unique_transactions
        FROM event_ledger 
        WHERE processed_at > NOW() - INTERVAL '1 minute';"
        
        Write-Host "Event Processing (last minute):" -ForegroundColor White
        Write-Host "  Events Processed: $($eventStats.Split('|')[0].Trim())" -ForegroundColor White
        Write-Host "  Unique Transactions: $($eventStats.Split('|')[1].Trim())" -ForegroundColor White
        
        # Daily summaries
        $summaryStats = docker exec lite-payment-processor-reconciliation-db-1 psql -U postgres -d reconciliation -t -c "
        SELECT 
            COUNT(*) as summaries_updated,
            SUM(total_transactions) as total_transactions,
            SUM(total_amount) as total_amount
        FROM daily_summaries 
        WHERE updated_at > NOW() - INTERVAL '1 minute';"
        
        Write-Host "`nDaily Summaries (last minute):" -ForegroundColor White
        Write-Host "  Summaries Updated: $($summaryStats.Split('|')[0].Trim())" -ForegroundColor White
        Write-Host "  Total Transactions: $($summaryStats.Split('|')[1].Trim())" -ForegroundColor White
        Write-Host "  Total Amount: $($summaryStats.Split('|')[2].Trim())" -ForegroundColor White
        
        # Anomalies
        $anomalyStats = docker exec lite-payment-processor-reconciliation-db-1 psql -U postgres -d reconciliation -t -c "
        SELECT 
            COUNT(*) as anomalies_detected,
            COUNT(CASE WHEN severity = 'HIGH' THEN 1 END) as high_severity,
            COUNT(CASE WHEN severity = 'CRITICAL' THEN 1 END) as critical_severity
        FROM anomalies 
        WHERE detected_at > NOW() - INTERVAL '1 minute';"
        
        Write-Host "`nAnomalies Detected (last minute):" -ForegroundColor White
        Write-Host "  Total: $($anomalyStats.Split('|')[0].Trim())" -ForegroundColor White
        Write-Host "  High Severity: $($anomalyStats.Split('|')[1].Trim())" -ForegroundColor Yellow
        Write-Host "  Critical Severity: $($anomalyStats.Split('|')[2].Trim())" -ForegroundColor Red
        
        # Database connections
        $connectionStats = docker exec lite-payment-processor-reconciliation-db-1 psql -U postgres -d reconciliation -t -c "
        SELECT 
            COUNT(*) as total_connections,
            COUNT(CASE WHEN state = 'active' THEN 1 END) as active_connections
        FROM pg_stat_activity 
        WHERE datname = 'reconciliation';"
        
        Write-Host "`nDatabase Connections:" -ForegroundColor White
        Write-Host "  Total: $($connectionStats.Split('|')[0].Trim())" -ForegroundColor White
        Write-Host "  Active: $($connectionStats.Split('|')[1].Trim())" -ForegroundColor Green
    }
}

function Show-ApplicationMetrics {
    Write-Host "`n--- Application Metrics ---" -ForegroundColor Magenta
    
    try {
        # Payment processor metrics
        $paymentMetrics = Invoke-RestMethod -Uri "http://localhost:3001/metrics" -TimeoutSec 5
        $paymentLines = $paymentMetrics -split "`n"
        
        Write-Host "Payment Processor Metrics:" -ForegroundColor White
        foreach ($line in $paymentLines) {
            if ($line -match "transactions_total|transactions_created_total|transactions_committed_total|transactions_failed_total|outbox_events_total") {
                Write-Host "  $line" -ForegroundColor Cyan
            }
        }
        
        # Reconciliation service metrics
        $reconciliationMetrics = Invoke-RestMethod -Uri "http://localhost:3002/metrics" -TimeoutSec 5
        $reconciliationLines = $reconciliationMetrics -split "`n"
        
        Write-Host "`nReconciliation Service Metrics:" -ForegroundColor White
        foreach ($line in $reconciliationLines) {
            if ($line -match "events_processed_total|anomalies_detected_total|daily_summaries_updated_total") {
                Write-Host "  $line" -ForegroundColor Cyan
            }
        }
    } catch {
        Write-Host "  Could not fetch application metrics (services may not be running)" -ForegroundColor Red
    }
}

function Show-ActiveQueries {
    Write-Host "`n--- Active Database Queries ---" -ForegroundColor Magenta
    
    if ($Database -eq "payment") {
        $activeQueries = docker exec lite-payment-processor-payment-db-1 psql -U postgres -d payment_processor -c "
        SELECT 
            pid,
            usename,
            application_name,
            state,
            query_start,
            LEFT(query, 100) as query_preview
        FROM pg_stat_activity 
        WHERE state = 'active' 
        AND query NOT LIKE '%pg_stat_activity%'
        ORDER BY query_start DESC;"
        
        Write-Host $activeQueries -ForegroundColor White
    } else {
        $activeQueries = docker exec lite-payment-processor-reconciliation-db-1 psql -U postgres -d reconciliation -c "
        SELECT 
            pid,
            usename,
            application_name,
            state,
            query_start,
            LEFT(query, 100) as query_preview
        FROM pg_stat_activity 
        WHERE state = 'active' 
        AND query NOT LIKE '%pg_stat_activity%'
        ORDER BY query_start DESC;"
        
        Write-Host $activeQueries -ForegroundColor White
    }
}

# Main monitoring loop
while ($true) {
    try {
        Get-DatabaseStats -db $Database
        
        if ($ShowMetrics) {
            Show-ApplicationMetrics
        }
        
        if ($ShowQueries) {
            Show-ActiveQueries
        }
        
        Write-Host "`n" + "="*50 -ForegroundColor Cyan
        
    } catch {
        Write-Host "Error occurred: $($_.Exception.Message)" -ForegroundColor Red
        Write-Host "Continuing monitoring..." -ForegroundColor Yellow
    }
    
    Start-Sleep -Seconds $IntervalSeconds
}

