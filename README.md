# DFSP-Lite Payments & Reconciliation Mini-Platform

A production-style, two-service mini-platform simulating a Digital Financial Services Provider (DFSP) gateway built with Rust.

## Overview

This project demonstrates microservices architecture, message-driven design, and resilience patterns within a distributed system. It consists of two independent services that collaborate through an event-driven workflow to handle payment processing and reconciliation.

## Architecture

```
┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│   Payment Processor │    │   Redis (Events)   │    │ Reconciliation &   │
│   (Service A)       │◄──►│   (Pub/Sub)        │◄──►│   Reporting        │
│   Port: 3001        │    │   Port: 6379       │    │   (Service B)       │
└─────────────────────┘    └─────────────────────┘    │   Port: 3002        │
         │                                           └─────────────────────┘
         ▼                                                    │
┌─────────────────────┐                                      ▼
│   PostgreSQL        │                              ┌─────────────────────┐
│   (Payment DB)      │                              │   PostgreSQL        │
│   Port: 5432        │                              │   (Reconciliation)  │
└─────────────────────┘                              │   Port: 5433        │
                                                     └─────────────────────┘
```

## System Flow Diagrams

### 1. High-Level System Architecture

```mermaid
graph TB
    subgraph "Client Layer"
        API[API Clients]
        Dashboard[Live Dashboard]
        LoadTest[Load Tests]
    end
    
    subgraph "Service Layer"
        PP[Payment Processor<br/>Port: 3001]
        RS[Reconciliation Service<br/>Port: 3002]
    end
    
    subgraph "Data Layer"
        Redis[(Redis<br/>Pub/Sub & Cache)]
        PDB[(Payment DB<br/>PostgreSQL)]
        RDB[(Reconciliation DB<br/>PostgreSQL)]
        SDB[(Staging DB<br/>PostgreSQL)]
    end
    
    subgraph "Monitoring"
        Prometheus[Prometheus]
        Grafana[Grafana]
        Metrics[Metrics Endpoints]
    end
    
    API --> PP
    Dashboard --> PP
    Dashboard --> RS
    LoadTest --> PP
    LoadTest --> RS
    
    PP --> Redis
    PP --> PDB
    RS --> Redis
    RS --> RDB
    RS --> SDB
    
    PP --> Metrics
    RS --> Metrics
    Metrics --> Prometheus
    Prometheus --> Grafana
```

### 2. Transaction Processing Flow

```mermaid
flowchart TD
    Start([Client Request]) --> Validate{Validate Request}
    Validate -->|Invalid| Error1[Return Error]
    Validate -->|Valid| CheckIdempotency{Check Idempotency}
    
    CheckIdempotency -->|Duplicate| Error2[Return Duplicate Error]
    CheckIdempotency -->|New| CreateTransaction[Create Transaction]
    
    CreateTransaction --> StoreDB[(Store in Database)]
    StoreDB --> EmitEvent[Emit Created Event]
    EmitEvent --> Outbox[Store in Outbox]
    Outbox --> SetLock[Set Idempotency Lock]
    SetLock --> Broadcast[Broadcast to WebSocket]
    Broadcast --> Success[Return Success]
    
    CreateTransaction -->|Error| Error3[Return Database Error]
    
    style Start fill:#e1f5fe
    style Success fill:#c8e6c9
    style Error1 fill:#ffcdd2
    style Error2 fill:#ffcdd2
    style Error3 fill:#ffcdd2
```

### 3. Event-Driven Reconciliation Flow

```mermaid
sequenceDiagram
    participant Client
    participant PP as Payment Processor
    participant Redis
    participant RS as Reconciliation Service
    participant PDB as Payment DB
    participant RDB as Reconciliation DB
    
    Client->>PP: POST /transactions
    PP->>PDB: Store Transaction
    PP->>Redis: Publish Event
    PP->>Client: Return Transaction ID
    
    Note over Redis: Event Processing
    Redis->>RS: Consume Event
    RS->>RDB: Store in Event Ledger
    RS->>RDB: Update Daily Summary
    
    Note over RS: Periodic Reconciliation
    RS->>PDB: Query Transaction Count
    RS->>RDB: Query Event Count
    RS->>RS: Compare & Detect Anomalies
    RS->>RDB: Store Anomalies (if any)
```

### 4. Safe Event Replay Process

```mermaid
flowchart TD
    Start([Start Safe Replay]) --> CreateBackup[Create Production Backup]
    CreateBackup --> ClearStaging[Clear Staging Database]
    ClearStaging --> GetEvents[Get All Events from Source]
    GetEvents --> ProcessEvents[Process Events to Staging]
    
    ProcessEvents --> ValidateData{Validate Staging Data}
    ValidateData -->|Invalid| Rollback[Rollback & Mark Failed]
    ValidateData -->|Valid| AtomicSwap[Atomic Table Swap]
    
    AtomicSwap --> UpdateProduction[Update Production Tables]
    UpdateProduction --> Cleanup[Cleanup Old Tables]
    Cleanup --> Complete[Mark Replay Complete]
    
    Rollback --> Error[Return Error]
    
    style Start fill:#e1f5fe
    style Complete fill:#c8e6c9
    style Rollback fill:#ffcdd2
    style Error fill:#ffcdd2
```

### 5. State Machine Flow

```mermaid
stateDiagram-v2
    [*] --> PENDING: Create Transaction
    
    PENDING --> COMMITTED: Commit Transaction
    PENDING --> FAILED: Fail Transaction
    PENDING --> CANCELLED: Cancel Transaction
    
    COMMITTED --> [*]: Transaction Complete
    FAILED --> [*]: Transaction Complete
    CANCELLED --> [*]: Transaction Complete
    
    note right of PENDING: Initial state after creation
    note right of COMMITTED: Successfully processed
    note right of FAILED: Processing failed
    note right of CANCELLED: Manually cancelled
```

### 6. Database Schema Relationships

```mermaid
erDiagram
    TRANSACTIONS {
        uuid id PK
        string external_id UK
        int64 amount
        string currency
        string state
        timestamp created_at
        timestamp updated_at
    }
    
    OUTBOX_EVENTS {
        uuid id PK
        uuid transaction_id FK
        string event_type
        jsonb event_data
        string status
        int32 retry_count
        timestamp created_at
    }
    
    EVENT_LEDGER {
        uuid event_id PK
        uuid transaction_id FK
        string event_type
        jsonb event_data
        timestamp processed_at
        timestamp created_at
    }
    
    DAILY_SUMMARIES {
        date date PK
        int64 total_transactions
        int64 total_amount
        int64 committed_count
        int64 failed_count
        int64 pending_count
        timestamp updated_at
    }
    
    RECONCILIATION_REPORTS {
        uuid report_id PK
        timestamp generated_at
        timestamp period_start
        timestamp period_end
        int64 total_transactions
        int64 total_amount
        int64 anomalies_count
        jsonb report_data
    }
    
    ANOMALIES {
        uuid anomaly_id PK
        uuid transaction_id FK
        string anomaly_type
        string description
        timestamp detected_at
        string severity
    }
    
    TRANSACTIONS ||--o{ OUTBOX_EVENTS : "generates"
    TRANSACTIONS ||--o{ EVENT_LEDGER : "tracked_in"
    EVENT_LEDGER ||--o{ DAILY_SUMMARIES : "aggregated_to"
    ANOMALIES ||--o{ RECONCILIATION_REPORTS : "included_in"
```

### 7. Monitoring & Observability Stack

```mermaid
graph TB
    subgraph "Application Layer"
        PP[Payment Processor]
        RS[Reconciliation Service]
    end
    
    subgraph "Metrics Collection"
        PP --> PP_Metrics[/metrics endpoint]
        RS --> RS_Metrics[/metrics endpoint]
    end
    
    subgraph "Monitoring Stack"
        PP_Metrics --> Prometheus[Prometheus<br/>Metrics Storage]
        RS_Metrics --> Prometheus
        Prometheus --> Grafana[Grafana<br/>Visualization]
    end
    
    subgraph "Health Monitoring"
        PP --> PP_Health[/health endpoint]
        RS --> RS_Health[/health endpoint]
    end
    
    subgraph "Logging"
        PP --> PP_Logs[Structured Logs]
        RS --> RS_Logs[Structured Logs]
    end
    
    style Prometheus fill:#e3f2fd
    style Grafana fill:#f3e5f5
```

### 8. Load Testing Flow

```mermaid
sequenceDiagram
    participant K6 as k6 Load Test
    participant PP as Payment Processor
    participant Redis
    participant RS as Reconciliation Service
    
    Note over K6: Load Test Execution
    loop For 60 seconds
        K6->>PP: POST /transactions (200 req/s)
        PP->>Redis: Publish Events
        PP->>K6: Response (p95 < 200ms)
    end
    
    Note over RS: Event Processing
    Redis->>RS: Consume Events
    RS->>RS: Process & Store Events
    
    Note over K6: Reconciliation Test
    K6->>RS: POST /reconcile
    RS->>RS: Run Reconciliation
    RS->>K6: Return Results
```

### 9. Error Handling & Resilience Patterns

```mermaid
flowchart TD
    Request([Incoming Request]) --> Validate{Input Validation}
    Validate -->|Invalid| ReturnError[Return 400 Bad Request]
    Validate -->|Valid| ProcessRequest[Process Request]
    
    ProcessRequest --> TryOperation[Try Database Operation]
    TryOperation -->|Success| Success[Return Success]
    TryOperation -->|Timeout| Retry{Retry Count < Max?}
    TryOperation -->|Connection Error| CircuitBreaker{Circuit Open?}
    
    Retry -->|Yes| Wait[Wait & Retry]
    Retry -->|No| ReturnTimeout[Return Timeout Error]
    Wait --> TryOperation
    
    CircuitBreaker -->|Open| ReturnCircuitOpen[Return Circuit Open Error]
    CircuitBreaker -->|Closed| TryOperation
    
    style ReturnError fill:#ffcdd2
    style ReturnTimeout fill:#ffcdd2
    style ReturnCircuitOpen fill:#ffcdd2
    style Success fill:#c8e6c9
```

### 10. Security & Authentication Flow

```mermaid
sequenceDiagram
    participant Client
    participant PP as Payment Processor
    participant Auth as Auth Service
    participant DB as Database
    
    Client->>PP: Request with JWT Token
    PP->>Auth: Validate JWT Token
    Auth->>DB: Check Token Validity
    DB->>Auth: Return Token Status
    Auth->>PP: Token Valid/Invalid
    
    alt Token Valid
        PP->>PP: Process Request
        PP->>Client: Return Response
    else Token Invalid
        PP->>Client: Return 401 Unauthorized
    end
```

## Services

### Service A - Payment Processor
- **Port**: 3001
- **Database**: PostgreSQL (port 5432)
- **Responsibilities**:
  - Multi-format input ingestion (JSON, ISO-8583-like)
  - Transaction state machine (PENDING → COMMITTED → FAILED)
  - Idempotency enforcement using Redis locks
  - Event publishing for state changes
  - JWT-based authentication

### Service B - Reconciliation & Reporting
- **Port**: 3002
- **Database**: PostgreSQL (port 5433)
- **Responsibilities**:
  - Event consumption from Payment Processor
  - Event-sourced ledger maintenance
  - Periodic reconciliation with transaction store
  - Anomaly detection and recording
  - Daily report generation (CSV format)

## Quick Start

### Prerequisites
- Docker and Docker Compose
- Rust 1.75+ (for local development)
- k6 (for load testing)

### Running with Docker Compose

1. **Clone and start services**:
   ```bash
   git clone <repository-url>
   cd dfsp-lite-payments
   docker-compose up -d
   ```

2. **Verify services are running**:
   ```bash
   # Check Payment Processor health
   curl http://localhost:3001/health
   
   # Check Reconciliation Service health
   curl http://localhost:3002/health
   ```

3. **Run load tests**:
   ```bash
   # Install k6 if not already installed
   # Windows: choco install k6
   # macOS: brew install k6
   # Linux: apt-get install k6
   
   # Run payment processor load test
   k6 run load-tests/load-test.js
   
   # Run reconciliation service load test
   k6 run load-tests/reconciliation-test.js
   ```

4. **Access the Live Dashboard**:
   ```bash
   # Open the dashboard in your browser
   open dashboard/index.html
   # Or navigate to: file:///path/to/dashboard/index.html
   ```

5. **View Prometheus Metrics**:
   ```bash
   # Payment Processor metrics
   curl http://localhost:3001/metrics
   
   # Reconciliation Service metrics
   curl http://localhost:3002/metrics
   
   # Prometheus UI (if running)
   open http://localhost:9090
   ```

### Local Development

1. **Start dependencies**:
   ```bash
   docker-compose up -d payment-db reconciliation-db redis
   ```

2. **Run migrations**:
   ```bash
   # Payment Processor DB
   psql -h localhost -p 5432 -U postgres -d payment_processor -f migrations/001_payment_processor_schema.sql
   
   # Reconciliation DB
   psql -h localhost -p 5433 -U postgres -d reconciliation -f migrations/002_reconciliation_schema.sql
   ```

3. **Build and run services**:
   ```bash
   # Build all services
   cargo build --release
   
   # Run Payment Processor
   cd payment-processor
   cargo run
   
   # Run Reconciliation Service (in another terminal)
   cd reconciliation-service
   cargo run
   ```

## API Documentation

### Payment Processor API (Port 3001)

#### Create Transaction
```http
POST /transactions
Content-Type: application/json

{
  "external_id": "unique-external-id",
  "amount": 10000,
  "currency": "USD",
  "from_account": "account-123",
  "to_account": "account-456",
  "description": "Payment description",
  "metadata": {
    "key": "value"
  }
}
```

#### Get Transaction
```http
GET /transactions/{transaction-id}
```

#### Update Transaction State
```http
POST /transactions/{transaction-id}/commit
POST /transactions/{transaction-id}/fail
POST /transactions/{transaction-id}/cancel
```

#### List Transactions
```http
GET /transactions?state=PENDING&limit=100&offset=0
```

### Reconciliation Service API (Port 3002)

#### Generate Report
```http
POST /reports/generate
Content-Type: application/json

{
  "period_start": "2024-01-01T00:00:00Z",
  "period_end": "2024-01-02T00:00:00Z"
}
```

#### List Reports
```http
GET /reports?limit=50&offset=0
```

#### Download Report (CSV)
```http
GET /reports/{report-id}/download
```

#### List Anomalies
```http
GET /anomalies?severity=HIGH&limit=100&offset=0
```

#### Trigger Reconciliation
```http
POST /reconcile
```

## Performance Requirements

- **Throughput**: 200 transactions per second for 60 seconds
- **Latency**: p95 < 200ms
- **Error Rate**: < 1%
- **Idempotency**: Duplicate requests handled correctly
- **Resilience**: Timeout, retry, circuit-breaker patterns

## Database Design

### Payment Processor Schema
- **Primary Index**: `idx_transactions_external_id` on `external_id` (most frequent query)
- **Performance Index**: `idx_transactions_state_created_at` for reconciliation queries
- **Optimization**: Composite index for state + time queries

### Reconciliation Schema
- **Event Ledger**: Event-sourced storage with transaction-based indexing
- **Reports**: Time-based partitioning for efficient report generation
- **Anomalies**: Severity-based indexing for quick anomaly retrieval

## Security

- **JWT Authentication**: Self-issued tokens for critical operations
- **Input Validation**: Comprehensive validation for all API inputs
- **SQL Injection Protection**: Parameterized queries using SQLx
- **Rate Limiting**: Redis-based rate limiting (can be added)

## Monitoring & Observability

- **Health Checks**: `/health` endpoints for both services
- **Structured Logging**: JSON-formatted logs with tracing
- **Metrics**: Request/response metrics (Prometheus-ready)
- **Distributed Tracing**: Request correlation across services

## Load Testing Results

Run the included k6 tests to verify performance requirements:

```bash
# Payment Processor Load Test
k6 run load-tests/load-test.js

# Reconciliation Service Load Test  
k6 run load-tests/reconciliation-test.js
```

Expected results:
- ✅ P95 Latency < 200ms
- ✅ Error Rate < 1%
- ✅ Target Throughput: 200+ req/s

## Project Structure

```
dfsp-lite-payments/
├── Cargo.toml                 # Workspace configuration
├── docker-compose.yml         # Docker services
├── migrations/                # Database schemas
│   ├── 001_payment_processor_schema.sql
│   └── 002_reconciliation_schema.sql
├── load-tests/               # k6 load tests
│   ├── load-test.js
│   └── reconciliation-test.js
├── shared/                   # Common types and utilities
│   ├── Cargo.toml
│   └── src/lib.rs
├── payment-processor/        # Service A
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── auth.rs
│       ├── database.rs
│       ├── redis_client.rs
│       └── state_machine.rs
├── reconciliation-service/   # Service B
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── database.rs
│       ├── redis_client.rs
│       └── reconciliation.rs
└── Dockerfile.*              # Service-specific Dockerfiles
```

## Development Guidelines

### Code Quality
- **Rust Best Practices**: Follow Rust idioms and conventions
- **Error Handling**: Use `anyhow` and `thiserror` for robust error handling
- **Testing**: Unit tests for business logic, integration tests for APIs
- **Documentation**: Comprehensive inline documentation

### Architecture Principles
- **Separation of Concerns**: Clear service boundaries
- **Event-Driven Design**: Loose coupling through events
- **Resilience Patterns**: Timeout, retry, circuit-breaker
- **Data Consistency**: Eventual consistency with reconciliation

## Troubleshooting

### Common Issues

1. **Database Connection Errors**:
   ```bash
   # Check if databases are running
   docker-compose ps
   
   # Check database logs
   docker-compose logs payment-db
   docker-compose logs reconciliation-db
   ```

2. **Redis Connection Issues**:
   ```bash
   # Test Redis connection
   redis-cli -h localhost -p 6379 ping
   ```

3. **Service Health Checks**:
   ```bash
   # Check service health
   curl http://localhost:3001/health
   curl http://localhost:3002/health
   ```

### Performance Tuning

1. **Database Optimization**:
   - Monitor query performance
   - Adjust connection pool sizes
   - Optimize indexes based on query patterns

2. **Redis Optimization**:
   - Monitor memory usage
   - Configure appropriate TTL values
   - Use Redis clustering for scale

## ✅ Completed Stretch Goals

- **✅ Outbox Pattern**: Transactional event publishing implemented with retry logic
- **✅ Event Replay**: Complete ledger reconstruction capability with progress tracking
- **✅ Prometheus + Grafana**: Comprehensive metrics collection and visualization
- **✅ Live Dashboard**: Real-time transaction monitoring with WebSocket support

## Future Enhancements

- **Kubernetes Deployment**: Production-ready orchestration
- **Advanced Analytics**: Machine learning-based anomaly detection
- **Multi-Region Support**: Geographic distribution and failover
- **API Rate Limiting**: Advanced rate limiting with Redis
- **Audit Logging**: Comprehensive audit trail with encryption

## License

This project is licensed under the MIT License - see the LICENSE file for details.




# Lite-Payment-Processor
