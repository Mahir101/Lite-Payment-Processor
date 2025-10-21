# Lite Payment Processor - Complete Documentation

## Table of Contents
1. [Project Overview](#project-overview)
2. [Architecture](#architecture)
3. [Services](#services)
4. [API Reference](#api-reference)
5. [Data Models](#data-models)
6. [Database Schema](#database-schema)
7. [Authentication](#authentication)
8. [Monitoring](#monitoring)
9. [Deployment](#deployment)
10. [Development](#development)
11. [Testing](#testing)
12. [Troubleshooting](#troubleshooting)

## Project Overview

The Lite Payment Processor is a comprehensive Rust-based microservices payment processing system designed for handling financial transactions with high reliability, monitoring, and reconciliation capabilities.

### Key Features
- **High Performance**: Built with Rust for maximum performance
- **Microservices Architecture**: Scalable service separation
- **Event Sourcing**: Complete transaction audit trail
- **Real-time Monitoring**: WebSocket-based live updates
- **Fraud Detection**: Built-in security measures
- **Reconciliation**: Automated transaction verification
- **Outbox Pattern**: Reliable event publishing
- **Card Validation**: Luhn algorithm and fraud detection
- **User Management**: Complete user and account management
- **Visa API Integration**: External payment processing

### Technology Stack
- **Language**: Rust
- **Web Framework**: Axum
- **Database**: PostgreSQL with SQLx
- **Cache**: Redis
- **Authentication**: JWT tokens
- **Monitoring**: Prometheus metrics
- **Real-time**: WebSocket connections
- **Containerization**: Docker & Docker Compose

## Architecture

### System Overview
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

### Service Communication
- **Event-Driven**: Services communicate via Redis pub/sub
- **Outbox Pattern**: Ensures reliable event delivery
- **Event Sourcing**: Complete audit trail of all transactions
- **Health Checks**: Service health monitoring
- **Metrics**: Prometheus-based observability

## Services

### Payment Processor Service (Port 3001)

**Core Responsibilities:**
- Transaction lifecycle management
- User and account management
- Card validation and fraud detection
- Payment processing via Visa API
- Real-time WebSocket updates
- JWT-based authentication

**Key Modules:**
- `main.rs` - Application entry point and routing
- `auth.rs` - JWT token management
- `card_validation.rs` - Card validation and fraud detection
- `database.rs` - Database operations
- `state_machine.rs` - Transaction state management
- `user_management.rs` - User and account operations
- `visa_api.rs` - Visa payment processing
- `websocket.rs` - Real-time communication
- `outbox.rs` - Event publishing
- `metrics.rs` - Performance monitoring
- `redis_client.rs` - Redis operations

### Reconciliation Service (Port 3002)

**Core Responsibilities:**
- Event consumption and processing
- Reconciliation report generation
- Anomaly detection
- Daily summary maintenance
- Event replay capabilities

**Key Modules:**
- `main.rs` - Service entry point
- `database.rs` - Reconciliation database operations
- `reconciliation.rs` - Reconciliation logic
- `event_replay.rs` - Event replay functionality
- `metrics.rs` - Service metrics
- `redis_client.rs` - Redis event consumption

### Shared Library

**Core Components:**
- Data structures and types
- Error handling
- API response formats
- Common utilities

## API Reference

### Payment Processor API (Port 3001)

#### Health & Monitoring
- `GET /health` - Service health check
- `GET /metrics` - Prometheus metrics
- `GET /ws` - WebSocket connection

#### Transaction Management
- `POST /transactions` - Create transaction
- `GET /transactions/:id` - Get transaction
- `POST /transactions/:id/commit` - Commit transaction
- `POST /transactions/:id/fail` - Fail transaction
- `POST /transactions/:id/cancel` - Cancel transaction
- `GET /transactions` - List transactions

#### User Management
- `POST /users` - Create user
- `GET /users/:id` - Get user
- `POST /users/:id/verify` - Verify user
- `GET /users/:id/accounts` - Get user accounts

#### Account Management
- `POST /accounts` - Create account
- `GET /accounts/:number` - Get account

#### Payment Processing
- `POST /transfer` - Transfer money
- `POST /validate-card` - Validate card
- `POST /visa-payment` - Process Visa payment

### Reconciliation Service API (Port 3002)

#### Health & Monitoring
- `GET /health` - Service health check
- `GET /metrics` - Prometheus metrics

#### Report Management
- `GET /reports` - List reports
- `GET /reports/:id` - Get report
- `GET /reports/:id/download` - Download CSV report
- `POST /reports/generate` - Generate report

#### Anomaly Management
- `GET /anomalies` - List anomalies
- `GET /anomalies/:id` - Get anomaly

#### Summary & Analysis
- `GET /daily-summaries` - Daily summaries
- `POST /reconcile` - Trigger reconciliation

#### Event Replay
- `POST /replay/start` - Start replay
- `GET /replay/:id` - Get replay status
- `GET /replay` - List replays

## Data Models

### Core Transaction Model
```rust
pub struct Transaction {
    pub id: Uuid,
    pub external_id: String,
    pub amount: i64, // Amount in cents
    pub currency: String,
    pub from_account: String,
    pub to_account: String,
    pub description: Option<String>,
    pub state: TransactionState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}
```

### Transaction States
```rust
pub enum TransactionState {
    Pending,    // Initial state
    Committed,  // Successfully completed
    Failed,     // Processing failed
    Cancelled,  // User cancelled
}
```

### Card Information
```rust
pub struct CardInfo {
    pub pan: String, // Primary Account Number
    pub expiry_month: u8,
    pub expiry_year: u16,
    pub cvv: String,
    pub cardholder_name: String,
    pub billing_address: BillingAddress,
}
```

### User Information
```rust
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub phone: Option<String>,
    pub device_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub is_verified: bool,
}
```

### Account Information
```rust
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
```

## Database Schema

### Payment Processor Database (Port 5432)

#### Tables
- `transactions` - Core transaction data
- `transaction_events` - Event sourcing events
- `users` - User information
- `accounts` - Account data
- `outbox_events` - Outbox pattern events

#### Key Indexes
- `idx_transactions_external_id` - External ID lookup
- `idx_transactions_state_created_at` - State and time queries
- `idx_users_email` - Email-based user lookup
- `idx_accounts_number` - Account number lookup

### Reconciliation Database (Port 5433)

#### Tables
- `reconciliation_reports` - Generated reports
- `anomalies` - Detected anomalies
- `daily_summaries` - Daily transaction summaries
- `event_replays` - Replay operation tracking

## Authentication

### JWT Token Structure
```json
{
  "sub": "user_123",
  "exp": 1642248600,
  "iat": 1642245000,
  "iss": "payment-processor",
  "aud": "dfsp-lite"
}
```

### Token Usage
- Include in `Authorization` header: `Bearer <token>`
- Tokens expire after 1 hour
- Refresh tokens as needed
- Validate tokens on protected endpoints

## Monitoring

### Prometheus Metrics

#### Transaction Metrics
- `transactions_total` - Total transactions processed
- `transactions_by_state` - Transactions by state
- `transaction_amount_total` - Total transaction amounts

#### HTTP Metrics
- `http_requests_total` - HTTP request counts
- `http_request_duration_seconds` - Request durations

#### System Metrics
- `database_connections_active` - Active DB connections
- `redis_operations_total` - Redis operation counts
- `errors_total` - Error counts by type

### Health Check Response
```json
{
  "success": true,
  "data": {
    "service": "payment-processor",
    "status": "Healthy",
    "timestamp": "2024-01-15T10:30:00Z",
    "version": "1.0.0",
    "dependencies": {
      "database": {
        "status": "Healthy",
        "response_time_ms": 10,
        "last_check": "2024-01-15T10:30:00Z"
      },
      "redis": {
        "status": "Healthy",
        "response_time_ms": 5,
        "last_check": "2024-01-15T10:30:00Z"
      }
    }
  }
}
```

## Deployment

### Docker Compose Setup
```bash
# Start all services
docker-compose up -d

# Start specific services
docker-compose up -d payment-processor reconciliation-service

# View logs
docker-compose logs -f payment-processor
```

### Environment Variables
```bash
# Database
DATABASE_URL=postgresql://postgres:password@localhost:5432/payment_processor

# Redis
REDIS_URL=redis://localhost:6379

# Authentication
JWT_SECRET=your-secret-key-change-in-production

# Visa API
VISA_API_KEY=your-visa-api-key
```

### Production Considerations
- Use connection pooling for database
- Configure Redis clustering for high availability
- Set up load balancing for multiple instances
- Implement proper monitoring and alerting
- Use secure secrets management
- Enable SSL/TLS for all communications
- Set up automated backups
- Configure log aggregation

## Development

### Local Development Setup
```bash
# Start dependencies
docker-compose up -d payment-db reconciliation-db redis

# Run migrations
psql -h localhost -p 5432 -U postgres -d payment_processor -f migrations/001_payment_processor_schema.sql
psql -h localhost -p 5433 -U postgres -d reconciliation -f migrations/002_reconciliation_schema.sql

# Build and run services
cargo build --release
cd payment-processor && cargo run
cd reconciliation-service && cargo run
```

### Code Structure
```
Lite-Payment-Processor/
├── Cargo.toml                 # Workspace configuration
├── docker-compose.yml         # Docker services
├── migrations/                # Database schemas
├── load-tests/               # k6 load tests
├── shared/                   # Common types and utilities
├── payment-processor/        # Service A
├── reconciliation-service/   # Service B
├── dashboard/                # Web dashboard
└── docs/                     # Documentation
```

## Testing

### Load Testing with k6
```bash
# Install k6
# Windows: choco install k6
# macOS: brew install k6
# Linux: apt-get install k6

# Run payment processor load test
k6 run load-tests/load-test.js

# Run reconciliation service load test
k6 run load-tests/reconciliation-test.js
```

### Performance Requirements
- **Throughput**: 200 transactions per second for 60 seconds
- **Latency**: p95 < 200ms
- **Error Rate**: < 1%
- **Idempotency**: Duplicate requests handled correctly

## Troubleshooting

### Common Issues

1. **Database Connection Errors**
   ```bash
   # Check if databases are running
   docker-compose ps
   
   # Check database logs
   docker-compose logs payment-db
   ```

2. **Redis Connection Issues**
   ```bash
   # Test Redis connection
   redis-cli -h localhost -p 6379 ping
   ```

3. **Service Health Checks**
   ```bash
   # Check service health
   curl http://localhost:3001/health
   curl http://localhost:3002/health
   ```

### Performance Tuning

1. **Database Optimization**
   - Monitor query performance
   - Adjust connection pool sizes
   - Optimize indexes based on query patterns

2. **Redis Optimization**
   - Monitor memory usage
   - Configure appropriate TTL values
   - Use Redis clustering for scale

## Security Best Practices

### Card Data Protection
- Never store full card numbers
- Use tokenization for card references
- Implement PCI DSS compliance
- Mask card numbers in logs
- Use secure transmission protocols

### Authentication Security
- Use strong JWT secrets
- Implement token rotation
- Validate all tokens server-side
- Use HTTPS for all communications
- Implement rate limiting

### API Security
- Implement input validation
- Use HTTPS everywhere
- Implement CORS properly
- Add request rate limiting
- Monitor for suspicious activity

## Future Enhancements

- **Kubernetes Deployment**: Production-ready orchestration
- **Advanced Analytics**: Machine learning-based anomaly detection
- **Multi-Region Support**: Geographic distribution and failover
- **API Rate Limiting**: Advanced rate limiting with Redis
- **Audit Logging**: Comprehensive audit trail with encryption

## License

This project is licensed under the MIT License - see the LICENSE file for details.
