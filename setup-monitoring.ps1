# PostgreSQL Database Monitoring Setup Script
# This script sets up the complete monitoring environment for Lite Payment Processor

param(
    [switch]$SetupDatabases = $false,
    [switch]$StartServices = $false,
    [switch]$RunMonitoring = $false,
    [switch]$GenerateTestData = $false,
    [switch]$OpenDashboard = $false,
    [switch]$All = $false
)

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "PostgreSQL Database Monitoring Setup" -ForegroundColor Cyan
Write-Host "Lite Payment Processor" -ForegroundColor Yellow
Write-Host "===============================================" -ForegroundColor Cyan

if ($All) {
    $SetupDatabases = $true
    $StartServices = $true
    $RunMonitoring = $true
    $GenerateTestData = $true
    $OpenDashboard = $true
}

function Test-Prerequisites {
    Write-Host "`nChecking prerequisites..." -ForegroundColor Yellow
    
    # Check if Docker is running
    try {
        docker version | Out-Null
        Write-Host "✓ Docker is running" -ForegroundColor Green
    } catch {
        Write-Host "✗ Docker is not running. Please start Docker Desktop." -ForegroundColor Red
        return $false
    }
    
    # Check if Rust/Cargo is available
    try {
        cargo --version | Out-Null
        Write-Host "✓ Rust/Cargo is available" -ForegroundColor Green
    } catch {
        Write-Host "✗ Rust/Cargo not found. Please install Rust." -ForegroundColor Red
        return $false
    }
    
    # Check if PowerShell execution policy allows scripts
    $executionPolicy = Get-ExecutionPolicy
    if ($executionPolicy -eq "Restricted") {
        Write-Host "⚠ PowerShell execution policy is restricted. Run: Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser" -ForegroundColor Yellow
    } else {
        Write-Host "✓ PowerShell execution policy allows scripts" -ForegroundColor Green
    }
    
    return $true
}

function Start-Databases {
    Write-Host "`nStarting PostgreSQL databases..." -ForegroundColor Yellow
    
    # Stop any existing containers
    Write-Host "Stopping existing containers..." -ForegroundColor Gray
    docker-compose down 2>$null
    
    # Start databases and Redis
    Write-Host "Starting databases and Redis..." -ForegroundColor Gray
    docker-compose up -d payment-db reconciliation-db redis
    
    # Wait for databases to be ready
    Write-Host "Waiting for databases to be ready..." -ForegroundColor Gray
    $maxAttempts = 30
    $attempt = 0
    
    do {
        Start-Sleep -Seconds 2
        $attempt++
        
        try {
            $paymentHealth = docker exec lite-payment-processor-payment-db-1 pg_isready -U postgres -d payment_processor 2>$null
            $reconciliationHealth = docker exec lite-payment-processor-reconciliation-db-1 pg_isready -U postgres -d reconciliation 2>$null
            
            if ($paymentHealth -match "accepting connections" -and $reconciliationHealth -match "accepting connections") {
                Write-Host "✓ Databases are ready!" -ForegroundColor Green
                break
            }
        } catch {
            # Continue waiting
        }
        
        Write-Host "." -NoNewline -ForegroundColor Gray
        
    } while ($attempt -lt $maxAttempts)
    
    if ($attempt -eq $maxAttempts) {
        Write-Host "`n✗ Databases failed to start within timeout" -ForegroundColor Red
        return $false
    }
    
    Write-Host "`n✓ Databases started successfully!" -ForegroundColor Green
    return $true
}

function Start-ApplicationServices {
    Write-Host "`nStarting application services..." -ForegroundColor Yellow
    
    # Kill any existing processes on ports
    Write-Host "Checking for existing processes..." -ForegroundColor Gray
    
    $ports = @(3001, 3002)
    foreach ($port in $ports) {
        $process = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue
        if ($process) {
            $pid = $process.OwningProcess
            Write-Host "Killing process on port $port (PID: $pid)" -ForegroundColor Yellow
            Stop-Process -Id $pid -Force -ErrorAction SilentlyContinue
        }
    }
    
    # Start payment processor
    Write-Host "Starting payment processor service..." -ForegroundColor Gray
    Start-Process -FilePath "cargo" -ArgumentList "run", "--bin", "payment-processor" -WindowStyle Minimized
    
    # Wait a moment for the service to start
    Start-Sleep -Seconds 3
    
    # Start reconciliation service
    Write-Host "Starting reconciliation service..." -ForegroundColor Gray
    Start-Process -FilePath "cargo" -ArgumentList "run", "--bin", "reconciliation-service" -WindowStyle Minimized
    
    # Wait for services to be ready
    Write-Host "Waiting for services to be ready..." -ForegroundColor Gray
    $maxAttempts = 20
    $attempt = 0
    
    do {
        Start-Sleep -Seconds 2
        $attempt++
        
        try {
            $paymentHealth = Invoke-RestMethod -Uri "http://localhost:3001/health" -TimeoutSec 5 -ErrorAction SilentlyContinue
            $reconciliationHealth = Invoke-RestMethod -Uri "http://localhost:3002/health" -TimeoutSec 5 -ErrorAction SilentlyContinue
            
            if ($paymentHealth -and $reconciliationHealth) {
                Write-Host "✓ Services are ready!" -ForegroundColor Green
                break
            }
        } catch {
            # Continue waiting
        }
        
        Write-Host "." -NoNewline -ForegroundColor Gray
        
    } while ($attempt -lt $maxAttempts)
    
    if ($attempt -eq $maxAttempts) {
        Write-Host "`n✗ Services failed to start within timeout" -ForegroundColor Red
        return $false
    }
    
    Write-Host "`n✓ Application services started successfully!" -ForegroundColor Green
    return $true
}

function Show-MonitoringOptions {
    Write-Host "`n===============================================" -ForegroundColor Cyan
    Write-Host "Database Monitoring Options" -ForegroundColor Cyan
    Write-Host "===============================================" -ForegroundColor Cyan
    
    Write-Host "`n1. PowerShell Monitoring Script:" -ForegroundColor Yellow
    Write-Host "   .\monitor-db.ps1 -Database payment" -ForegroundColor White
    Write-Host "   .\monitor-db.ps1 -Database reconciliation" -ForegroundColor White
    Write-Host "   .\monitor-db.ps1 -Database payment -ShowMetrics -ShowQueries" -ForegroundColor White
    
    Write-Host "`n2. Test Data Generation:" -ForegroundColor Yellow
    Write-Host "   .\generate-test-data.ps1 -TransactionCount 10" -ForegroundColor White
    Write-Host "   .\generate-test-data.ps1 -Continuous" -ForegroundColor White
    
    Write-Host "`n3. Direct Database Access:" -ForegroundColor Yellow
    Write-Host "   docker exec -it lite-payment-processor-payment-db-1 psql -U postgres -d payment_processor" -ForegroundColor White
    Write-Host "   docker exec -it lite-payment-processor-reconciliation-db-1 psql -U postgres -d reconciliation" -ForegroundColor White
    
    Write-Host "`n4. SQL Monitoring Queries:" -ForegroundColor Yellow
    Write-Host "   See monitoring-queries.sql for comprehensive monitoring queries" -ForegroundColor White
    
    Write-Host "`n5. Web Dashboard:" -ForegroundColor Yellow
    Write-Host "   Open dashboard/index.html in your browser" -ForegroundColor White
    Write-Host "   Use the Database Monitoring section for real-time metrics" -ForegroundColor White
    
    Write-Host "`n6. Application Metrics:" -ForegroundColor Yellow
    Write-Host "   http://localhost:3001/metrics (Payment Processor)" -ForegroundColor White
    Write-Host "   http://localhost:3002/metrics (Reconciliation Service)" -ForegroundColor White
}

function Open-Dashboard {
    Write-Host "`nOpening dashboard..." -ForegroundColor Yellow
    
    $dashboardPath = Join-Path $PSScriptRoot "dashboard\index.html"
    if (Test-Path $dashboardPath) {
        Start-Process $dashboardPath
        Write-Host "✓ Dashboard opened in browser" -ForegroundColor Green
    } else {
        Write-Host "✗ Dashboard not found at: $dashboardPath" -ForegroundColor Red
    }
}

function Generate-SampleData {
    Write-Host "`nGenerating sample test data..." -ForegroundColor Yellow
    
    if (Test-Path ".\generate-test-data.ps1") {
        & .\generate-test-data.ps1 -TransactionCount 5
        Write-Host "✓ Sample data generated" -ForegroundColor Green
    } else {
        Write-Host "✗ Test data generation script not found" -ForegroundColor Red
    }
}

# Main execution
if (-not (Test-Prerequisites)) {
    Write-Host "`nPrerequisites check failed. Please fix the issues above and try again." -ForegroundColor Red
    exit 1
}

if ($SetupDatabases) {
    if (-not (Start-Databases)) {
        Write-Host "`nDatabase setup failed. Please check Docker and try again." -ForegroundColor Red
        exit 1
    }
}

if ($StartServices) {
    if (-not (Start-ApplicationServices)) {
        Write-Host "`nService startup failed. Please check the logs and try again." -ForegroundColor Red
        exit 1
    }
}

if ($GenerateTestData) {
    Generate-SampleData
}

if ($OpenDashboard) {
    Open-Dashboard
}

Show-MonitoringOptions

Write-Host "`n===============================================" -ForegroundColor Cyan
Write-Host "Setup Complete!" -ForegroundColor Green
Write-Host "===============================================" -ForegroundColor Cyan

Write-Host "`nNext Steps:" -ForegroundColor Yellow
Write-Host "1. Use the monitoring scripts to watch database activity" -ForegroundColor White
Write-Host "2. Generate test data to see updates in real-time" -ForegroundColor White
Write-Host "3. Open the dashboard for visual monitoring" -ForegroundColor White
Write-Host "4. Check the monitoring-queries.sql for advanced queries" -ForegroundColor White

Write-Host "`nFor help, run: .\setup-monitoring.ps1 -All" -ForegroundColor Cyan

