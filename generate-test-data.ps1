# Test Data Generation Script for Lite Payment Processor
# This script generates test transactions and data to monitor database updates

param(
    [int]$TransactionCount = 10,
    [int]$IntervalSeconds = 2,
    [switch]$Continuous = $false,
    [string]$Currency = "USD",
    [int]$MinAmount = 100,
    [int]$MaxAmount = 10000
)

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "Test Data Generator for Database Monitoring" -ForegroundColor Cyan
Write-Host "Transactions: $TransactionCount" -ForegroundColor Yellow
Write-Host "Interval: $IntervalSeconds seconds" -ForegroundColor Yellow
Write-Host "Currency: $Currency" -ForegroundColor Yellow
Write-Host "Amount Range: $MinAmount - $MaxAmount cents" -ForegroundColor Yellow
Write-Host "Continuous Mode: $Continuous" -ForegroundColor Yellow
Write-Host "Press Ctrl+C to stop" -ForegroundColor Red
Write-Host "===============================================" -ForegroundColor Cyan

# Test account numbers
$testAccounts = @(
    "account-001", "account-002", "account-003", "account-004", "account-005",
    "account-006", "account-007", "account-008", "account-009", "account-010"
)

# Test descriptions
$testDescriptions = @(
    "Payment for services",
    "Transfer to savings",
    "Online purchase",
    "Bill payment",
    "Peer-to-peer transfer",
    "Subscription payment",
    "Refund processing",
    "Salary deposit",
    "Investment transfer",
    "Emergency payment"
)

function Test-ServiceHealth {
    try {
        $health = Invoke-RestMethod -Uri "http://localhost:3001/health" -TimeoutSec 5
        return $true
    } catch {
        Write-Host "Payment processor service is not running or not healthy" -ForegroundColor Red
        return $false
    }
}

function Generate-RandomTransaction {
    $timestamp = Get-Date -Format "yyyyMMddHHmmss"
    $randomSuffix = Get-Random -Minimum 1000 -Maximum 9999
    
    $fromAccount = $testAccounts | Get-Random
    $toAccount = $testAccounts | Get-Random
    
    # Ensure from and to accounts are different
    while ($fromAccount -eq $toAccount) {
        $toAccount = $testAccounts | Get-Random
    }
    
    $amount = Get-Random -Minimum $MinAmount -Maximum $MaxAmount
    $description = $testDescriptions | Get-Random
    
    return @{
        external_id = "test-$timestamp-$randomSuffix"
        amount = $amount
        currency = $Currency
        from_account = $fromAccount
        to_account = $toAccount
        description = $description
        metadata = @{
            test_run = $true
            generated_at = (Get-Date).ToString("yyyy-MM-ddTHH:mm:ssZ")
            generator_version = "1.0"
        }
    }
}

function Send-Transaction {
    param([hashtable]$transaction)
    
    try {
        $json = $transaction | ConvertTo-Json -Depth 3
        $response = Invoke-RestMethod -Uri "http://localhost:3001/transactions" -Method POST -Body $json -ContentType "application/json" -TimeoutSec 10
        
        Write-Host "✓ Transaction created: $($transaction.external_id) - $($transaction.amount) $($transaction.currency)" -ForegroundColor Green
        Write-Host "  From: $($transaction.from_account) To: $($transaction.to_account)" -ForegroundColor Gray
        Write-Host "  Description: $($transaction.description)" -ForegroundColor Gray
        
        return $response
    } catch {
        Write-Host "✗ Failed to create transaction: $($transaction.external_id)" -ForegroundColor Red
        Write-Host "  Error: $($_.Exception.Message)" -ForegroundColor Red
        return $null
    }
}

function Simulate-TransactionLifecycle {
    param([string]$transactionId)
    
    Start-Sleep -Seconds (Get-Random -Minimum 1 -Maximum 3)
    
    try {
        # Simulate transaction processing by checking status
        $status = Invoke-RestMethod -Uri "http://localhost:3001/transactions/$transactionId" -TimeoutSec 5
        Write-Host "  Status: $($status.state)" -ForegroundColor Cyan
        
        # Randomly commit or fail some transactions
        if ((Get-Random -Minimum 1 -Maximum 10) -le 8) { # 80% success rate
            try {
                $commitResponse = Invoke-RestMethod -Uri "http://localhost:3001/transactions/$transactionId/commit" -Method POST -TimeoutSec 5
                Write-Host "  ✓ Transaction committed: $transactionId" -ForegroundColor Green
            } catch {
                Write-Host "  ✗ Failed to commit transaction: $transactionId" -ForegroundColor Red
            }
        } else {
            try {
                $failResponse = Invoke-RestMethod -Uri "http://localhost:3001/transactions/$transactionId/fail" -Method POST -TimeoutSec 5
                Write-Host "  ✗ Transaction failed: $transactionId" -ForegroundColor Red
            } catch {
                Write-Host "  ✗ Failed to fail transaction: $transactionId" -ForegroundColor Red
            }
        }
    } catch {
        Write-Host "  ✗ Could not process transaction lifecycle: $transactionId" -ForegroundColor Red
    }
}

function Show-Statistics {
    Write-Host "`n--- Current Statistics ---" -ForegroundColor Magenta
    
    try {
        # Get recent transactions from database
        $recentTransactions = docker exec lite-payment-processor-payment-db-1 psql -U postgres -d payment_processor -t -c "
        SELECT 
            COUNT(*) as total,
            COUNT(CASE WHEN state = 'PENDING' THEN 1 END) as pending,
            COUNT(CASE WHEN state = 'COMMITTED' THEN 1 END) as committed,
            COUNT(CASE WHEN state = 'FAILED' THEN 1 END) as failed,
            COUNT(CASE WHEN external_id LIKE 'test-%' THEN 1 END) as test_transactions
        FROM transactions 
        WHERE created_at > NOW() - INTERVAL '1 hour';"
        
        $stats = $recentTransactions.Split('|')
        Write-Host "Recent Transactions (last hour):" -ForegroundColor White
        Write-Host "  Total: $($stats[0].Trim())" -ForegroundColor White
        Write-Host "  Pending: $($stats[1].Trim())" -ForegroundColor Yellow
        Write-Host "  Committed: $($stats[2].Trim())" -ForegroundColor Green
        Write-Host "  Failed: $($stats[3].Trim())" -ForegroundColor Red
        Write-Host "  Test Transactions: $($stats[4].Trim())" -ForegroundColor Cyan
        
    } catch {
        Write-Host "Could not fetch statistics from database" -ForegroundColor Red
    }
}

# Main execution
Write-Host "`nChecking service health..." -ForegroundColor Yellow

if (-not (Test-ServiceHealth)) {
    Write-Host "Please start the payment processor service first:" -ForegroundColor Red
    Write-Host "  cargo run --bin payment-processor" -ForegroundColor Yellow
    exit 1
}

Write-Host "Service is healthy. Starting test data generation..." -ForegroundColor Green

$createdTransactions = @()
$successCount = 0
$failureCount = 0

if ($Continuous) {
    Write-Host "`nRunning in continuous mode..." -ForegroundColor Yellow
    $counter = 0
    
    while ($true) {
        $counter++
        Write-Host "`n--- Batch $counter ---" -ForegroundColor Cyan
        
        for ($i = 1; $i -le $TransactionCount; $i++) {
            $transaction = Generate-RandomTransaction
            $response = Send-Transaction -transaction $transaction
            
            if ($response) {
                $createdTransactions += $response
                $successCount++
                
                # Simulate transaction lifecycle in background
                Start-Job -ScriptBlock {
                    param($id)
                    Simulate-TransactionLifecycle -transactionId $id
                } -ArgumentList $response.id | Out-Null
            } else {
                $failureCount++
            }
            
            if ($i -lt $TransactionCount) {
                Start-Sleep -Seconds $IntervalSeconds
            }
        }
        
        Show-Statistics
        
        Write-Host "`nWaiting before next batch..." -ForegroundColor Yellow
        Start-Sleep -Seconds 10
    }
} else {
    Write-Host "`nGenerating $TransactionCount test transactions..." -ForegroundColor Yellow
    
    for ($i = 1; $i -le $TransactionCount; $i++) {
        Write-Host "`nTransaction $i/$TransactionCount" -ForegroundColor Cyan
        
        $transaction = Generate-RandomTransaction
        $response = Send-Transaction -transaction $transaction
        
        if ($response) {
            $createdTransactions += $response
            $successCount++
            
            # Simulate transaction lifecycle
            Simulate-TransactionLifecycle -transactionId $response.id
        } else {
            $failureCount++
        }
        
        if ($i -lt $TransactionCount) {
            Start-Sleep -Seconds $IntervalSeconds
        }
    }
    
    Write-Host "`n" + "="*50 -ForegroundColor Cyan
    Write-Host "Test Data Generation Complete!" -ForegroundColor Green
    Write-Host "Successfully created: $successCount transactions" -ForegroundColor Green
    Write-Host "Failed to create: $failureCount transactions" -ForegroundColor Red
    
    Show-Statistics
    
    Write-Host "`nYou can now monitor the database updates using:" -ForegroundColor Yellow
    Write-Host "  .\monitor-db.ps1 -Database payment" -ForegroundColor Cyan
    Write-Host "  .\monitor-db.ps1 -Database reconciliation" -ForegroundColor Cyan
    Write-Host "  .\monitor-db.ps1 -Database payment -ShowMetrics -ShowQueries" -ForegroundColor Cyan
}

# Cleanup background jobs
Get-Job | Remove-Job -Force

