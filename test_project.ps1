# DFSP-Lite Payment Platform Test Script
# This script tests the project components without Docker

Write-Host "=== DFSP-Lite Payment Platform Test ===" -ForegroundColor Green

# Check if PostgreSQL is running
Write-Host "`n1. Checking PostgreSQL..." -ForegroundColor Yellow
try {
    $pgTest = psql -h localhost -U postgres -d postgres -c "SELECT version();" 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ PostgreSQL is running" -ForegroundColor Green
    } else {
        Write-Host "❌ PostgreSQL is not running or not accessible" -ForegroundColor Red
        Write-Host "Please start PostgreSQL service or install it" -ForegroundColor Yellow
        exit 1
    }
} catch {
    Write-Host "❌ PostgreSQL is not installed or not in PATH" -ForegroundColor Red
    exit 1
}

# Check if Redis is available (optional for basic testing)
Write-Host "`n2. Checking Redis..." -ForegroundColor Yellow
try {
    $redisTest = redis-cli ping 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Redis is running" -ForegroundColor Green
        $hasRedis = $true
    } else {
        Write-Host "⚠️ Redis is not running - some features will be limited" -ForegroundColor Yellow
        $hasRedis = $false
    }
} catch {
    Write-Host "⚠️ Redis is not installed - some features will be limited" -ForegroundColor Yellow
    $hasRedis = $false
}

# Create test databases
Write-Host "`n3. Setting up test databases..." -ForegroundColor Yellow
try {
    # Create payment processor database
    psql -h localhost -U postgres -d postgres -c "CREATE DATABASE payment_processor;" 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Created payment_processor database" -ForegroundColor Green
    } else {
        Write-Host "⚠️ payment_processor database might already exist" -ForegroundColor Yellow
    }
    
    # Create reconciliation database
    psql -h localhost -U postgres -d postgres -c "CREATE DATABASE reconciliation;" 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Created reconciliation database" -ForegroundColor Green
    } else {
        Write-Host "⚠️ reconciliation database might already exist" -ForegroundColor Yellow
    }
} catch {
    Write-Host "❌ Failed to create databases" -ForegroundColor Red
    exit 1
}

# Run database migrations
Write-Host "`n4. Running database migrations..." -ForegroundColor Yellow
try {
    # Payment processor migration
    Get-Content "migrations/001_payment_processor_schema.sql" | psql -h localhost -U postgres -d payment_processor
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Payment processor migration completed" -ForegroundColor Green
    } else {
        Write-Host "❌ Payment processor migration failed" -ForegroundColor Red
        exit 1
    }
    
    # Reconciliation migration
    Get-Content "migrations/002_reconciliation_schema.sql" | psql -h localhost -U postgres -d reconciliation
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Reconciliation migration completed" -ForegroundColor Green
    } else {
        Write-Host "❌ Reconciliation migration failed" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "❌ Failed to run migrations" -ForegroundColor Red
    exit 1
}

# Test compilation
Write-Host "`n5. Testing compilation..." -ForegroundColor Yellow
try {
    cargo build --release
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Compilation successful" -ForegroundColor Green
    } else {
        Write-Host "❌ Compilation failed" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "❌ Failed to compile project" -ForegroundColor Red
    exit 1
}

# Test API endpoints (if services can start)
Write-Host "`n6. Testing API endpoints..." -ForegroundColor Yellow
Write-Host "Note: This requires manual testing by starting the services" -ForegroundColor Cyan
Write-Host "`nTo test the services manually:" -ForegroundColor Cyan
Write-Host "1. Start Payment Processor: cargo run --bin payment-processor" -ForegroundColor White
Write-Host "2. Start Reconciliation Service: cargo run --bin reconciliation-service" -ForegroundColor White
Write-Host "3. Test endpoints with curl or Postman" -ForegroundColor White

# Show test commands
Write-Host "`n=== Test Commands ===" -ForegroundColor Green
Write-Host "`nPayment Processor Tests:" -ForegroundColor Yellow
Write-Host "curl -X POST http://localhost:3001/transactions -H 'Content-Type: application/json' -d '{\"external_id\":\"test123\",\"amount\":10000,\"currency\":\"USD\",\"from_account\":\"acc1\",\"to_account\":\"acc2\"}'" -ForegroundColor White
Write-Host "curl http://localhost:3001/transactions/test123" -ForegroundColor White
Write-Host "curl http://localhost:3001/health" -ForegroundColor White
Write-Host "curl http://localhost:3001/metrics" -ForegroundColor White

Write-Host "`nReconciliation Service Tests:" -ForegroundColor Yellow
Write-Host "curl http://localhost:3002/health" -ForegroundColor White
Write-Host "curl http://localhost:3002/metrics" -ForegroundColor White
Write-Host "curl -X POST http://localhost:3002/replay/start" -ForegroundColor White

Write-Host "`nLive Dashboard:" -ForegroundColor Yellow
Write-Host "Open dashboard/index.html in your browser" -ForegroundColor White
Write-Host "WebSocket will connect to ws://localhost:3001/ws" -ForegroundColor White

Write-Host "`n=== Test Summary ===" -ForegroundColor Green
Write-Host "✅ Compilation: PASSED" -ForegroundColor Green
Write-Host "✅ Database Setup: PASSED" -ForegroundColor Green
if ($hasRedis) {
    Write-Host "✅ Redis: AVAILABLE" -ForegroundColor Green
} else {
    Write-Host "⚠️ Redis: NOT AVAILABLE (limited functionality)" -ForegroundColor Yellow
}
Write-Host "⚠️ Manual Testing: REQUIRED" -ForegroundColor Yellow

Write-Host "`nThe project is ready for testing! Start the services and test the endpoints." -ForegroundColor Green

