# Architecture Documentation

## System Overview

The Lite Payment Processor is a microservices-based payment processing system built with Rust, designed for high performance, reliability, and scalability. The system follows event-driven architecture principles with comprehensive monitoring and reconciliation capabilities.

## Architecture Principles

### 1. Microservices Architecture
- **Service Separation**: Clear separation of concerns between payment processing and reconciliation
- **Independent Deployment**: Each service can be deployed and scaled independently
- **Technology Diversity**: Each service can use different technologies if needed
- **Fault Isolation**: Failures in one service don't affect others

### 2. Event-Driven Architecture
- **Event Sourcing**: Complete audit trail of all transactions
- **Pub/Sub Communication**: Services communicate via Redis pub/sub
- **Outbox Pattern**: Reliable event publishing with database transactions
- **Event Replay**: Ability to reprocess historical events

### 3. Domain-Driven Design
- **Bounded Contexts**: Clear boundaries between payment and reconciliation domains
- **Aggregates**: Transaction and User aggregates with business logic
- **Value Objects**: CardInfo, BillingAddress, etc.
- **Domain Events**: Transaction lifecycle events

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                                Client Layer                                    │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Web Dashboard  │  Mobile App  │  API Clients  │  Monitoring Tools  │  Load Balancer │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              API Gateway Layer                                 │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Authentication  │  Rate Limiting  │  Request Routing  │  Response Aggregation │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            Microservices Layer                                 │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Payment Processor Service  │  Reconciliation Service  │  Shared Library      │
│  (Port 3001)                │  (Port 3002)             │  (Common Types)      │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            Communication Layer                                  │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Redis Pub/Sub  │  WebSocket  │  HTTP REST APIs  │  Event Streaming  │  Metrics │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              Data Layer                                        │
├─────────────────────────────────────────────────────────────────────────────────┤
│  PostgreSQL (Payment)  │  PostgreSQL (Reconciliation)  │  Redis Cache  │  Files │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Service Architecture

### Payment Processor Service

#### Core Components

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                          Payment Processor Service                             │
├─────────────────────────────────────────────────────────────────────────────────┤
│  HTTP Router (Actix-web)                                                        │
│  ├── Transaction Endpoints                                                      │
│  ├── User Management Endpoints                                                  │
│  ├── Account Management Endpoints                                               │
│  ├── Payment Processing Endpoints                                               │
│  └── Health & Monitoring Endpoints                                              │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Business Logic Layer                                                           │
│  ├── Transaction State Machine                                                  │
│  ├── Card Validation & Fraud Detection                                         │
│  ├── User & Account Management                                                  │
│  ├── Visa API Integration                                                       │
│  └── Authentication Service                                                     │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Data Access Layer                                                              │
│  ├── Database Service (SQLx)                                                   │
│  ├── Redis Service                                                              │
│  ├── Outbox Service                                                             │
│  └── WebSocket Manager                                                          │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Infrastructure Layer                                                           │
│  ├── Prometheus Metrics                                                         │
│  ├── Structured Logging                                                         │
│  ├── Health Checks                                                              │
│  └── Error Handling                                                             │
└─────────────────────────────────────────────────────────────────────────────────┘
```

#### Key Modules

1. **Transaction Management**
   - State machine for transaction lifecycle
   - Idempotency handling
   - Event publishing
   - Real-time updates

2. **Card Validation**
   - Luhn algorithm validation
   - Expiry date validation
   - CVV validation
   - Fraud detection
   - Card type detection

3. **User Management**
   - User creation and verification
   - Account management
   - Money transfers
   - Balance validation

4. **Payment Processing**
   - Visa API integration
   - External payment processing
   - Transaction validation
   - Error handling

### Reconciliation Service

#### Core Components

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                        Reconciliation Service                                  │
├─────────────────────────────────────────────────────────────────────────────────┤
│  HTTP Router (Actix-web)                                                        │
│  ├── Report Management Endpoints                                               │
│  ├── Anomaly Management Endpoints                                              │
│  ├── Summary & Analysis Endpoints                                              │
│  ├── Event Replay Endpoints                                                    │
│  └── Health & Monitoring Endpoints                                             │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Business Logic Layer                                                           │
│  ├── Reconciliation Engine                                                      │
│  ├── Anomaly Detection                                                          │
│  ├── Report Generation                                                          │
│  ├── Event Replay Service                                                       │
│  └── Daily Summary Service                                                      │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Data Access Layer                                                              │
│  ├── Database Service (SQLx)                                                   │
│  ├── Redis Service                                                              │
│  ├── Event Storage                                                              │
│  └── Report Storage                                                             │
├─────────────────────────────────────────────────────────────────────────────────┤
│  Infrastructure Layer                                                           │
│  ├── Prometheus Metrics                                                         │
│  ├── Structured Logging                                                         │
│  ├── Health Checks                                                              │
│  └── Error Handling                                                             │
└─────────────────────────────────────────────────────────────────────────────────┘
```

#### Key Modules

1. **Event Consumption**
   - Redis pub/sub subscription
   - Event processing pipeline
   - Error handling and retry logic
   - Event storage

2. **Reconciliation Engine**
   - Transaction validation
   - Balance reconciliation
   - Anomaly detection
   - Report generation

3. **Anomaly Detection**
   - Pattern recognition
   - Statistical analysis
   - Threshold monitoring
   - Alert generation

4. **Event Replay**
   - Historical event processing
   - Data recovery
   - Testing and validation
   - Audit trail reconstruction

## Data Architecture

### Database Design

#### Payment Processor Database

```sql
-- Core transaction table
CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    external_id VARCHAR(255) UNIQUE NOT NULL,
    amount BIGINT NOT NULL,
    currency VARCHAR(3) NOT NULL,
    from_account VARCHAR(255) NOT NULL,
    to_account VARCHAR(255) NOT NULL,
    description TEXT,
    state VARCHAR(20) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    metadata JSONB
);

-- Event sourcing table
CREATE TABLE transaction_events (
    id UUID PRIMARY KEY,
    transaction_id UUID NOT NULL REFERENCES transactions(id),
    event_type VARCHAR(50) NOT NULL,
    event_data JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- User management
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    phone VARCHAR(20),
    device_id VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    is_verified BOOLEAN DEFAULT FALSE
);

-- Account management
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    account_number VARCHAR(255) UNIQUE NOT NULL,
    balance BIGINT NOT NULL DEFAULT 0,
    currency VARCHAR(3) NOT NULL,
    account_type VARCHAR(20) NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Outbox pattern for reliable event publishing
CREATE TABLE outbox_events (
    id UUID PRIMARY KEY,
    aggregate_id UUID NOT NULL,
    aggregate_type VARCHAR(50) NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    event_data JSONB NOT NULL,
    status VARCHAR(20) DEFAULT 'PENDING',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    processed_at TIMESTAMP WITH TIME ZONE
);
```

#### Reconciliation Database

```sql
-- Reconciliation reports
CREATE TABLE reconciliation_reports (
    report_id UUID PRIMARY KEY,
    generated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    period_start TIMESTAMP WITH TIME ZONE NOT NULL,
    period_end TIMESTAMP WITH TIME ZONE NOT NULL,
    total_transactions INTEGER NOT NULL,
    total_amount BIGINT NOT NULL,
    anomalies JSONB DEFAULT '[]'::jsonb
);

-- Anomaly detection
CREATE TABLE anomalies (
    anomaly_id UUID PRIMARY KEY,
    transaction_id UUID,
    anomaly_type VARCHAR(50) NOT NULL,
    description TEXT NOT NULL,
    detected_at TIMESTAMP WITH TIME ZONE NOT NULL,
    severity VARCHAR(20) NOT NULL
);

-- Daily summaries
CREATE TABLE daily_summaries (
    date DATE PRIMARY KEY,
    total_transactions INTEGER NOT NULL,
    total_amount BIGINT NOT NULL,
    committed_count INTEGER NOT NULL,
    failed_count INTEGER NOT NULL,
    cancelled_count INTEGER NOT NULL
);

-- Event replay tracking
CREATE TABLE event_replays (
    replay_id UUID PRIMARY KEY,
    started_at TIMESTAMP WITH TIME ZONE NOT NULL,
    completed_at TIMESTAMP WITH TIME ZONE,
    status VARCHAR(20) NOT NULL,
    events_processed INTEGER DEFAULT 0,
    errors_count INTEGER DEFAULT 0
);
```

### Redis Usage

#### Caching Strategy

```redis
# Idempotency keys
SET txn:external_id_123 "transaction_uuid" EX 300

# Distributed locks
SET lock:account_123 "lock_value" EX 30 NX

# Session data
HSET session:user_123 "last_activity" "2024-01-15T10:30:00Z"
HSET session:user_123 "permissions" "read,write"

# Rate limiting
INCR rate_limit:ip_192.168.1.1
EXPIRE rate_limit:ip_192.168.1.1 60
```

#### Pub/Sub Channels

```redis
# Transaction events
PUBLISH transaction_events '{"type":"created","transaction_id":"uuid","data":{}}'
PUBLISH transaction_events '{"type":"committed","transaction_id":"uuid","data":{}}'
PUBLISH transaction_events '{"type":"failed","transaction_id":"uuid","data":{}}'

# System events
PUBLISH system_events '{"type":"user_created","user_id":"uuid","data":{}}'
PUBLISH system_events '{"type":"account_created","account_id":"uuid","data":{}}'
```

## Communication Patterns

### 1. Synchronous Communication

#### HTTP REST APIs
- **Request-Response**: Direct API calls between services
- **Load Balancing**: Multiple service instances
- **Circuit Breaker**: Fault tolerance for external calls
- **Retry Logic**: Exponential backoff for failed requests

#### WebSocket Connections
- **Real-time Updates**: Live transaction status updates
- **Connection Management**: Automatic reconnection
- **Message Broadcasting**: One-to-many communication
- **Heartbeat**: Connection health monitoring

### 2. Asynchronous Communication

#### Event-Driven Architecture
- **Event Publishing**: Outbox pattern for reliability
- **Event Consumption**: Redis pub/sub for decoupling
- **Event Sourcing**: Complete audit trail
- **Event Replay**: Historical event processing

#### Message Patterns
- **Command**: Transaction creation, user management
- **Event**: Transaction state changes, system events
- **Query**: Data retrieval, reporting
- **Saga**: Distributed transaction coordination

## Security Architecture

### 1. Authentication & Authorization

#### JWT Token Management
- **Token Generation**: HMAC-SHA256 signing
- **Token Validation**: Signature verification
- **Token Expiration**: 1-hour default lifetime
- **Token Refresh**: Automatic renewal mechanism

#### Access Control
- **Role-Based Access**: User permissions
- **Resource-Based Access**: Account-level permissions
- **API Key Management**: Service-to-service authentication
- **Rate Limiting**: Request throttling

### 2. Data Protection

#### Card Data Security
- **PCI DSS Compliance**: Industry-standard security
- **Tokenization**: Card number replacement
- **Masking**: Sensitive data protection
- **Encryption**: Data at rest and in transit

#### Fraud Detection
- **Pattern Recognition**: Suspicious activity detection
- **Risk Scoring**: Transaction risk assessment
- **Blocked Cards**: Fraud prevention
- **User Verification**: Identity validation

## Monitoring & Observability

### 1. Metrics Collection

#### Prometheus Metrics
- **Business Metrics**: Transaction counts, amounts, success rates
- **Technical Metrics**: Response times, error rates, throughput
- **Infrastructure Metrics**: CPU, memory, disk usage
- **Custom Metrics**: Domain-specific measurements

#### Metric Types
- **Counters**: Total transactions, errors
- **Gauges**: Active connections, queue sizes
- **Histograms**: Response time distributions
- **Summaries**: Quantile calculations

### 2. Logging Strategy

#### Structured Logging
- **JSON Format**: Machine-readable logs
- **Correlation IDs**: Request tracing
- **Log Levels**: Debug, info, warn, error
- **Contextual Information**: User, transaction, service data

#### Log Aggregation
- **Centralized Logging**: Single log store
- **Log Rotation**: Automatic cleanup
- **Log Analysis**: Pattern detection
- **Alerting**: Anomaly notifications

### 3. Health Monitoring

#### Health Checks
- **Liveness Probes**: Service availability
- **Readiness Probes**: Service readiness
- **Dependency Checks**: Database, Redis connectivity
- **Custom Health Checks**: Business logic validation

#### Alerting
- **Threshold-Based**: Metric value alerts
- **Anomaly Detection**: Statistical alerts
- **Error Rate Monitoring**: Failure alerts
- **Performance Degradation**: Latency alerts

## Deployment Architecture

### 1. Containerization

#### Docker Containers
- **Multi-stage Builds**: Optimized image sizes
- **Base Images**: Alpine Linux for security
- **Health Checks**: Container health monitoring
- **Resource Limits**: CPU and memory constraints

#### Docker Compose
- **Service Orchestration**: Multi-container applications
- **Network Configuration**: Service communication
- **Volume Management**: Data persistence
- **Environment Variables**: Configuration management

### 2. Service Discovery

#### Service Registration
- **Service Registry**: Centralized service directory
- **Health Reporting**: Service status updates
- **Load Balancing**: Traffic distribution
- **Failover**: Automatic service switching

#### Configuration Management
- **Environment Variables**: Runtime configuration
- **Configuration Files**: Static configuration
- **Secrets Management**: Secure credential storage
- **Dynamic Configuration**: Runtime updates

### 3. Scaling Strategy

#### Horizontal Scaling
- **Load Balancing**: Traffic distribution
- **Auto-scaling**: Dynamic instance management
- **Service Mesh**: Inter-service communication
- **Circuit Breakers**: Fault tolerance

#### Vertical Scaling
- **Resource Allocation**: CPU and memory increases
- **Performance Tuning**: Database optimization
- **Caching**: Redis optimization
- **Connection Pooling**: Database connections

## Performance Architecture

### 1. Throughput Optimization

#### Database Optimization
- **Connection Pooling**: Efficient connection management
- **Query Optimization**: Index usage, query planning
- **Read Replicas**: Read scaling
- **Partitioning**: Data distribution

#### Caching Strategy
- **Redis Caching**: Frequently accessed data
- **Application Caching**: In-memory caching
- **CDN**: Static content delivery
- **Cache Invalidation**: Data consistency

### 2. Latency Optimization

#### Network Optimization
- **Connection Reuse**: HTTP keep-alive
- **Compression**: Gzip compression
- **CDN**: Geographic distribution
- **Edge Computing**: Reduced latency

#### Application Optimization
- **Async Processing**: Non-blocking operations
- **Batch Processing**: Bulk operations
- **Connection Pooling**: Reduced connection overhead
- **Memory Management**: Efficient memory usage

## Disaster Recovery

### 1. Backup Strategy

#### Database Backups
- **Point-in-Time Recovery**: Transaction log backups
- **Full Backups**: Complete database snapshots
- **Incremental Backups**: Changed data only
- **Cross-Region Replication**: Geographic redundancy

#### Application Backups
- **Configuration Backups**: Service configuration
- **Code Backups**: Source code versioning
- **Data Backups**: Application data
- **State Backups**: Service state

### 2. Failover Strategy

#### High Availability
- **Active-Passive**: Standby instances
- **Active-Active**: Multiple active instances
- **Load Balancing**: Traffic distribution
- **Health Monitoring**: Automatic failover

#### Recovery Procedures
- **RTO**: Recovery Time Objective
- **RPO**: Recovery Point Objective
- **Backup Restoration**: Data recovery
- **Service Restoration**: Application recovery

## Future Architecture Considerations

### 1. Scalability Enhancements

#### Microservices Evolution
- **Service Mesh**: Advanced inter-service communication
- **API Gateway**: Centralized API management
- **Event Streaming**: Kafka integration
- **CQRS**: Command Query Responsibility Segregation

#### Cloud-Native Features
- **Kubernetes**: Container orchestration
- **Service Mesh**: Istio integration
- **Serverless**: Function-as-a-Service
- **Multi-Region**: Geographic distribution

### 2. Technology Evolution

#### Modern Frameworks
- **gRPC**: High-performance RPC
- **GraphQL**: Flexible data querying
- **WebAssembly**: High-performance computing
- **Edge Computing**: Reduced latency

#### Data Technologies
- **Time Series Databases**: Metrics storage
- **Graph Databases**: Relationship modeling
- **Stream Processing**: Real-time data processing
- **Machine Learning**: Predictive analytics

This architecture documentation provides a comprehensive overview of the Lite Payment Processor system, covering all aspects from high-level design to implementation details. The system is designed to be scalable, maintainable, and reliable while following industry best practices for payment processing systems.