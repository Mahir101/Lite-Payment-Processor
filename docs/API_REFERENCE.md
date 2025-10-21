# API Reference Documentation

## Payment Processor Service API (Port 3001)

### Base URL
```
http://localhost:3001
```

### Authentication
All endpoints require JWT authentication via the `Authorization` header:
```
Authorization: Bearer <jwt_token>
```

---

## Health & Monitoring Endpoints

### GET /health
**Description**: Service health check endpoint for monitoring and load balancers

**Response**:
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
  },
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /metrics
**Description**: Prometheus metrics endpoint for monitoring

**Response**: Prometheus-formatted metrics

### GET /ws
**Description**: WebSocket connection for real-time updates

**Usage**: Upgrade HTTP connection to WebSocket for live transaction updates

---

## Transaction Management Endpoints

### POST /transactions
**Description**: Creates a new payment transaction

**Request Body**:
```json
{
  "external_id": "txn_12345",
  "amount": 10000,
  "currency": "USD",
  "from_account": "ACC1234567890",
  "to_account": "ACC0987654321",
  "description": "Payment for services",
  "metadata": {
    "customer_id": "cust_123",
    "order_id": "order_456"
  }
}
```

**Response**:
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "external_id": "txn_12345",
    "amount": 10000,
    "currency": "USD",
    "from_account": "ACC1234567890",
    "to_account": "ACC0987654321",
    "description": "Payment for services",
    "state": "PENDING",
    "created_at": "2024-01-15T10:30:00Z",
    "updated_at": "2024-01-15T10:30:00Z",
    "metadata": {
      "customer_id": "cust_123",
      "order_id": "order_456"
    }
  },
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /transactions/{id}
**Description**: Retrieves a transaction by its UUID

**Path Parameters**:
- `id`: Transaction UUID

**Response**: Same as POST /transactions response

### POST /transactions/{id}/commit
**Description**: Commits a pending transaction to completed state

**Path Parameters**:
- `id`: Transaction UUID

**Response**: Updated transaction with COMMITTED state

### POST /transactions/{id}/fail
**Description**: Marks a transaction as failed

**Path Parameters**:
- `id`: Transaction UUID

**Response**: Updated transaction with FAILED state

### POST /transactions/{id}/cancel
**Description**: Cancels a pending transaction

**Path Parameters**:
- `id`: Transaction UUID

**Response**: Updated transaction with CANCELLED state

### GET /transactions
**Description**: Lists transactions with optional filtering

**Query Parameters**:
- `state`: Filter by transaction state (PENDING, COMMITTED, FAILED, CANCELLED)
- `limit`: Maximum number of transactions to return (default: 100, max: 1000)
- `offset`: Number of transactions to skip (default: 0)

**Response**:
```json
{
  "success": true,
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "external_id": "txn_12345",
      "amount": 10000,
      "currency": "USD",
      "from_account": "ACC1234567890",
      "to_account": "ACC0987654321",
      "description": "Payment for services",
      "state": "PENDING",
      "created_at": "2024-01-15T10:30:00Z",
      "updated_at": "2024-01-15T10:30:00Z",
      "metadata": {}
    }
  ],
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## User Management Endpoints

### POST /users
**Description**: Creates a new user account

**Request Body**:
```json
{
  "email": "john@example.com",
  "phone": "+1234567890",
  "device_id": "device_123"
}
```

**Response**:
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "john@example.com",
    "phone": "+1234567890",
    "device_id": "device_123",
    "created_at": "2024-01-15T10:30:00Z",
    "is_verified": false
  },
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /users/{id}
**Description**: Retrieves a user by their UUID

**Path Parameters**:
- `id`: User UUID

**Response**: Same as POST /users response

### POST /users/{id}/verify
**Description**: Verifies a user account

**Path Parameters**:
- `id`: User UUID

**Response**:
```json
{
  "success": true,
  "data": null,
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /users/{id}/accounts
**Description**: Retrieves all accounts for a user

**Path Parameters**:
- `id`: User UUID

**Response**:
```json
{
  "success": true,
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "user_id": "550e8400-e29b-41d4-a716-446655440000",
      "account_number": "ACC1234567890",
      "balance": 100000,
      "currency": "USD",
      "account_type": "CHECKING",
      "is_active": true,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## Account Management Endpoints

### POST /accounts
**Description**: Creates a new financial account

**Request Body**:
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "account_type": "CHECKING",
  "currency": "USD",
  "initial_balance": 100000
}
```

**Response**:
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "account_number": "ACC1234567890",
    "balance": 100000,
    "currency": "USD",
    "account_type": "CHECKING",
    "is_active": true,
    "created_at": "2024-01-15T10:30:00Z"
  },
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /accounts/{number}
**Description**: Retrieves an account by its account number

**Path Parameters**:
- `number`: Account number

**Response**: Same as POST /accounts response

---

## Payment Processing Endpoints

### POST /transfer
**Description**: Processes a money transfer between accounts

**Request Body**:
```json
{
  "from_account": "ACC1234567890",
  "to_account": "ACC0987654321",
  "amount": 5000,
  "currency": "USD",
  "description": "Transfer to savings",
  "card_info": {
    "pan": "4242424242424242",
    "expiry_month": 12,
    "expiry_year": 2025,
    "cvv": "123",
    "cardholder_name": "John Doe",
    "billing_address": {
      "line1": "123 Main St",
      "city": "New York",
      "postal_code": "10001",
      "country": "US"
    }
  },
  "user_info": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "john@example.com",
    "phone": "+1234567890",
    "device_id": "device_123",
    "created_at": "2024-01-01T00:00:00Z",
    "is_verified": true
  }
}
```

**Response**:
```json
{
  "success": true,
  "data": null,
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### POST /validate-card
**Description**: Validates a payment card

**Request Body**:
```json
{
  "pan": "4242424242424242",
  "expiry_month": 12,
  "expiry_year": 2025,
  "cvv": "123",
  "cardholder_name": "John Doe",
  "billing_address": {
    "line1": "123 Main St",
    "city": "New York",
    "postal_code": "10001",
    "country": "US"
  }
}
```

**Response**:
```json
{
  "success": true,
  "data": {
    "valid": true,
    "card_type": "Visa",
    "masked_pan": "************4242",
    "message": "Card is valid"
  },
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### POST /visa-payment
**Description**: Processes a payment through Visa API

**Request Body**:
```json
{
  "card_info": {
    "pan": "4242424242424242",
    "expiry_month": 12,
    "expiry_year": 2025,
    "cvv": "123",
    "cardholder_name": "John Doe",
    "billing_address": {
      "line1": "123 Main St",
      "city": "New York",
      "postal_code": "10001",
      "country": "US"
    }
  },
  "amount": 10000,
  "currency": "USD",
  "description": "Payment for services"
}
```

**Response**:
```json
{
  "success": true,
  "data": {
    "success": true,
    "visa_transaction_id": "visa_550e8400-e29b-41d4-a716-446655440000",
    "auth_code": "AUTH123456",
    "status": "APPROVED",
    "response_code": "00",
    "response_message": "Transaction approved",
    "amount": "100.00",
    "currency": "USD",
    "timestamp": "2024-01-15T10:30:00Z",
    "card_type": "Visa",
    "masked_card": "************4242"
  },
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## Reconciliation Service API (Port 3002)

### Base URL
```
http://localhost:3002
```

---

## Health & Monitoring Endpoints

### GET /health
**Description**: Service health check endpoint

**Response**: Same format as Payment Processor health check

### GET /metrics
**Description**: Prometheus metrics endpoint

**Response**: Prometheus-formatted metrics

---

## Report Management Endpoints

### POST /reports/generate
**Description**: Generates a reconciliation report for a specific period

**Request Body**:
```json
{
  "period_start": "2024-01-01T00:00:00Z",
  "period_end": "2024-01-02T00:00:00Z"
}
```

**Response**:
```json
{
  "success": true,
  "data": {
    "report_id": "550e8400-e29b-41d4-a716-446655440000",
    "generated_at": "2024-01-15T10:30:00Z",
    "period_start": "2024-01-01T00:00:00Z",
    "period_end": "2024-01-02T00:00:00Z",
    "total_transactions": 1500,
    "total_amount": 15000000,
    "anomalies": []
  },
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /reports
**Description**: Lists reconciliation reports

**Query Parameters**:
- `limit`: Maximum number of reports to return (default: 50, max: 1000)
- `offset`: Number of reports to skip (default: 0)

**Response**:
```json
{
  "success": true,
  "data": [
    {
      "report_id": "550e8400-e29b-41d4-a716-446655440000",
      "generated_at": "2024-01-15T10:30:00Z",
      "period_start": "2024-01-01T00:00:00Z",
      "period_end": "2024-01-02T00:00:00Z",
      "total_transactions": 1500,
      "total_amount": 15000000,
      "anomalies": []
    }
  ],
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /reports/{id}
**Description**: Retrieves a specific reconciliation report

**Path Parameters**:
- `id`: Report UUID

**Response**: Same as POST /reports/generate response

### GET /reports/{id}/download
**Description**: Downloads a reconciliation report in CSV format

**Path Parameters**:
- `id`: Report UUID

**Response**: CSV-formatted report data

---

## Anomaly Management Endpoints

### GET /anomalies
**Description**: Lists detected anomalies

**Query Parameters**:
- `severity`: Filter by severity (LOW, MEDIUM, HIGH, CRITICAL)
- `limit`: Maximum number of anomalies to return (default: 100, max: 1000)
- `offset`: Number of anomalies to skip (default: 0)

**Response**:
```json
{
  "success": true,
  "data": [
    {
      "anomaly_id": "550e8400-e29b-41d4-a716-446655440000",
      "transaction_id": "550e8400-e29b-41d4-a716-446655440000",
      "anomaly_type": "AmountMismatch",
      "description": "Transaction amount does not match ledger entry",
      "detected_at": "2024-01-15T10:30:00Z",
      "severity": "HIGH"
    }
  ],
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /anomalies/{id}
**Description**: Retrieves a specific anomaly

**Path Parameters**:
- `id`: Anomaly UUID

**Response**: Same as GET /anomalies response (single item)

---

## Summary & Analysis Endpoints

### GET /daily-summaries
**Description**: Retrieves daily transaction summaries

**Query Parameters**:
- `limit`: Maximum number of summaries to return (default: 30, max: 365)
- `offset`: Number of summaries to skip (default: 0)

**Response**:
```json
{
  "success": true,
  "data": [
    {
      "date": "2024-01-15",
      "total_transactions": 1500,
      "total_amount": 15000000,
      "committed_count": 1400,
      "failed_count": 50,
      "cancelled_count": 50
    }
  ],
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### POST /reconcile
**Description**: Triggers manual reconciliation process

**Response**:
```json
{
  "success": true,
  "data": "Reconciliation completed successfully",
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## Event Replay Endpoints

### POST /replay/start
**Description**: Starts a full event replay process

**Response**:
```json
{
  "success": true,
  "data": "550e8400-e29b-41d4-a716-446655440000",
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /replay/{id}
**Description**: Retrieves the status of an event replay

**Path Parameters**:
- `id`: Replay UUID

**Response**:
```json
{
  "success": true,
  "data": {
    "replay_id": "550e8400-e29b-41d4-a716-446655440000",
    "started_at": "2024-01-15T10:30:00Z",
    "completed_at": "2024-01-15T10:35:00Z",
    "status": "COMPLETED",
    "events_processed": 1500,
    "errors_count": 0
  },
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### GET /replay
**Description**: Lists all event replays

**Query Parameters**:
- `limit`: Maximum number of replays to return (default: 50, max: 1000)
- `offset`: Number of replays to skip (default: 0)

**Response**:
```json
{
  "success": true,
  "data": [
    {
      "replay_id": "550e8400-e29b-41d4-a716-446655440000",
      "started_at": "2024-01-15T10:30:00Z",
      "completed_at": "2024-01-15T10:35:00Z",
      "status": "COMPLETED",
      "events_processed": 1500,
      "errors_count": 0
    }
  ],
  "error": null,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

---

## Error Responses

### Error Format
All error responses follow this format:
```json
{
  "success": false,
  "data": null,
  "error": "Error message description",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### HTTP Status Codes
- `200 OK` - Successful operation
- `400 Bad Request` - Invalid request data
- `401 Unauthorized` - Authentication required
- `404 Not Found` - Resource not found
- `409 Conflict` - Duplicate transaction
- `500 Internal Server Error` - Server error

### Common Error Types
- `InvalidFormat` - Data format errors
- `TransactionNotFound` - Missing transactions
- `InvalidStateTransition` - State machine errors
- `DuplicateTransaction` - Idempotency violations
- `DatabaseError` - Database operation failures
- `RedisError` - Cache operation failures
- `AuthError` - Authentication failures
- `InvalidCard` - Card validation errors
- `InsufficientFunds` - Balance validation errors
- `FraudDetected` - Security violations

---

## Rate Limiting

Currently, no rate limiting is implemented. In production, consider implementing:
- Request rate limiting per IP
- API key-based rate limiting
- User-based rate limiting
- Endpoint-specific rate limits

---

## WebSocket Events

### Connection
Connect to `/ws` endpoint to receive real-time updates.

### Event Types

#### Transaction Created
```json
{
  "type": "transaction_created",
  "transaction": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "external_id": "txn_12345",
    "amount": 10000,
    "currency": "USD",
    "state": "PENDING",
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

#### Transaction Updated
```json
{
  "type": "transaction_updated",
  "transaction": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "external_id": "txn_12345",
    "amount": 10000,
    "currency": "USD",
    "state": "COMMITTED",
    "updated_at": "2024-01-15T10:35:00Z"
  }
}
```

#### Metrics Update
```json
{
  "type": "metrics_update",
  "metrics": {
    "total_transactions": 1500,
    "pending_transactions": 25,
    "committed_transactions": 1400,
    "failed_transactions": 50,
    "total_amount": 15000000,
    "throughput": 200.5,
    "avg_latency": 150.2,
    "p95_latency": 180.5,
    "error_rate": 0.01
  }
}
```
