# DFSP-Lite Payments - API Documentation

## Payment Processor Service (Port 3001)

### Base URL
```
http://localhost:3001
```

### Authentication
All endpoints require JWT authentication in the `Authorization` header:
```
Authorization: Bearer <jwt-token>
```

### Endpoints

#### Health Check
```http
GET /health
```

**Response:**
```json
{
  "success": true,
  "data": {
    "service": "payment-processor",
    "status": "Healthy",
    "timestamp": "2024-01-01T12:00:00Z",
    "version": "0.1.0",
    "dependencies": {
      "database": {
        "status": "Healthy",
        "response_time_ms": 10,
        "last_check": "2024-01-01T12:00:00Z"
      },
      "redis": {
        "status": "Healthy", 
        "response_time_ms": 5,
        "last_check": "2024-01-01T12:00:00Z"
      }
    }
  },
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Create Transaction
```http
POST /transactions
Content-Type: application/json
```

**Request Body:**
```json
{
  "external_id": "unique-external-id-123",
  "amount": 10000,
  "currency": "USD",
  "from_account": "account-123",
  "to_account": "account-456",
  "description": "Payment for services",
  "metadata": {
    "customer_id": "cust-789",
    "reference": "REF-001"
  }
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "external_id": "unique-external-id-123",
    "amount": 10000,
    "currency": "USD",
    "from_account": "account-123",
    "to_account": "account-456",
    "description": "Payment for services",
    "state": "PENDING",
    "created_at": "2024-01-01T12:00:00Z",
    "updated_at": "2024-01-01T12:00:00Z",
    "metadata": {
      "customer_id": "cust-789",
      "reference": "REF-001"
    }
  },
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

**Error Responses:**
- `400 Bad Request`: Invalid input data
- `409 Conflict`: Duplicate external_id (idempotency violation)
- `500 Internal Server Error`: System error

#### Get Transaction
```http
GET /transactions/{transaction-id}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "external_id": "unique-external-id-123",
    "amount": 10000,
    "currency": "USD",
    "from_account": "account-123",
    "to_account": "account-456",
    "description": "Payment for services",
    "state": "COMMITTED",
    "created_at": "2024-01-01T12:00:00Z",
    "updated_at": "2024-01-01T12:05:00Z",
    "metadata": {
      "customer_id": "cust-789",
      "reference": "REF-001"
    }
  },
  "error": null,
  "timestamp": "2024-01-01T12:05:00Z"
}
```

**Error Responses:**
- `404 Not Found`: Transaction not found
- `500 Internal Server Error`: System error

#### Commit Transaction
```http
POST /transactions/{transaction-id}/commit
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "external_id": "unique-external-id-123",
    "amount": 10000,
    "currency": "USD",
    "from_account": "account-123",
    "to_account": "account-456",
    "description": "Payment for services",
    "state": "COMMITTED",
    "created_at": "2024-01-01T12:00:00Z",
    "updated_at": "2024-01-01T12:05:00Z",
    "metadata": {
      "customer_id": "cust-789",
      "reference": "REF-001"
    }
  },
  "error": null,
  "timestamp": "2024-01-01T12:05:00Z"
}
```

**Error Responses:**
- `400 Bad Request`: Invalid state transition
- `404 Not Found`: Transaction not found
- `500 Internal Server Error`: System error

#### Fail Transaction
```http
POST /transactions/{transaction-id}/fail
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "external_id": "unique-external-id-123",
    "amount": 10000,
    "currency": "USD",
    "from_account": "account-123",
    "to_account": "account-456",
    "description": "Payment for services",
    "state": "FAILED",
    "created_at": "2024-01-01T12:00:00Z",
    "updated_at": "2024-01-01T12:05:00Z",
    "metadata": {
      "customer_id": "cust-789",
      "reference": "REF-001"
    }
  },
  "error": null,
  "timestamp": "2024-01-01T12:05:00Z"
}
```

#### Cancel Transaction
```http
POST /transactions/{transaction-id}/cancel
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "external_id": "unique-external-id-123",
    "amount": 10000,
    "currency": "USD",
    "from_account": "account-123",
    "to_account": "account-456",
    "description": "Payment for services",
    "state": "CANCELLED",
    "created_at": "2024-01-01T12:00:00Z",
    "updated_at": "2024-01-01T12:05:00Z",
    "metadata": {
      "customer_id": "cust-789",
      "reference": "REF-001"
    }
  },
  "error": null,
  "timestamp": "2024-01-01T12:05:00Z"
}
```

#### List Transactions
```http
GET /transactions?state=PENDING&limit=100&offset=0
```

**Query Parameters:**
- `state` (optional): Filter by transaction state (PENDING, COMMITTED, FAILED, CANCELLED)
- `limit` (optional): Number of transactions to return (default: 100, max: 1000)
- `offset` (optional): Number of transactions to skip (default: 0)

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "external_id": "unique-external-id-123",
      "amount": 10000,
      "currency": "USD",
      "from_account": "account-123",
      "to_account": "account-456",
      "description": "Payment for services",
      "state": "PENDING",
      "created_at": "2024-01-01T12:00:00Z",
      "updated_at": "2024-01-01T12:00:00Z",
      "metadata": {
        "customer_id": "cust-789",
        "reference": "REF-001"
      }
    }
  ],
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### WebSocket Connection
```javascript
// Connect to WebSocket for real-time updates
const ws = new WebSocket('ws://localhost:3001/ws');

ws.onmessage = function(event) {
    const data = JSON.parse(event.data);
    
    switch(data.type) {
        case 'transaction_created':
            console.log('New transaction:', data.transaction);
            break;
        case 'transaction_updated':
            console.log('Transaction updated:', data.transaction);
            break;
        case 'metrics_update':
            console.log('Metrics updated:', data.metrics);
            break;
    }
};
```

#### Get Metrics (Prometheus)
```http
GET /metrics
```

**Response:**
```
# HELP transactions_total Total number of transactions processed
# TYPE transactions_total counter
transactions_total 1500

# HELP transactions_created_total Total number of transactions created
# TYPE transactions_created_total counter
transactions_created_total 1200

# HELP http_requests_total Total number of HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="POST",endpoint="/transactions",status="200"} 1200
```

---

## Reconciliation Service (Port 3002)

### Base URL
```
http://localhost:3002
```

### Endpoints

#### Health Check
```http
GET /health
```

**Response:**
```json
{
  "success": true,
  "data": {
    "service": "reconciliation-service",
    "status": "Healthy",
    "timestamp": "2024-01-01T12:00:00Z",
    "version": "0.1.0",
    "dependencies": {
      "database": {
        "status": "Healthy",
        "response_time_ms": 15,
        "last_check": "2024-01-01T12:00:00Z"
      },
      "redis": {
        "status": "Healthy",
        "response_time_ms": 8,
        "last_check": "2024-01-01T12:00:00Z"
      }
    }
  },
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Generate Report
```http
POST /reports/generate
Content-Type: application/json
```

**Request Body:**
```json
{
  "period_start": "2024-01-01T00:00:00Z",
  "period_end": "2024-01-02T00:00:00Z"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "report_id": "660e8400-e29b-41d4-a716-446655440000",
    "generated_at": "2024-01-01T12:00:00Z",
    "period_start": "2024-01-01T00:00:00Z",
    "period_end": "2024-01-02T00:00:00Z",
    "total_transactions": 1500,
    "total_amount": 15000000,
    "anomalies": [
      {
        "anomaly_id": "770e8400-e29b-41d4-a716-446655440000",
        "transaction_id": null,
        "anomaly_type": "MissingTransaction",
        "description": "Sample anomaly for demonstration",
        "detected_at": "2024-01-01T12:00:00Z",
        "severity": "Medium"
      }
    ]
  },
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### List Reports
```http
GET /reports?limit=50&offset=0
```

**Query Parameters:**
- `limit` (optional): Number of reports to return (default: 50, max: 1000)
- `offset` (optional): Number of reports to skip (default: 0)

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "report_id": "660e8400-e29b-41d4-a716-446655440000",
      "generated_at": "2024-01-01T12:00:00Z",
      "period_start": "2024-01-01T00:00:00Z",
      "period_end": "2024-01-02T00:00:00Z",
      "total_transactions": 1500,
      "total_amount": 15000000,
      "anomalies": []
    }
  ],
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Get Report
```http
GET /reports/{report-id}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "report_id": "660e8400-e29b-41d4-a716-446655440000",
    "generated_at": "2024-01-01T12:00:00Z",
    "period_start": "2024-01-01T00:00:00Z",
    "period_end": "2024-01-02T00:00:00Z",
    "total_transactions": 1500,
    "total_amount": 15000000,
    "anomalies": []
  },
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Download Report (CSV)
```http
GET /reports/{report-id}/download
```

**Response:**
```
Transaction ID,Event Type,Timestamp,Amount,Status
550e8400-e29b-41d4-a716-446655440000,Created,2024-01-01 12:00:00 UTC,10000,PENDING
550e8400-e29b-41d4-a716-446655440000,StateChanged,2024-01-01 12:05:00 UTC,10000,COMMITTED
```

#### List Anomalies
```http
GET /anomalies?severity=HIGH&limit=100&offset=0
```

**Query Parameters:**
- `severity` (optional): Filter by severity (LOW, MEDIUM, HIGH, CRITICAL)
- `limit` (optional): Number of anomalies to return (default: 100, max: 1000)
- `offset` (optional): Number of anomalies to skip (default: 0)

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "anomaly_id": "770e8400-e29b-41d4-a716-446655440000",
      "transaction_id": "550e8400-e29b-41d4-a716-446655440000",
      "anomaly_type": "AmountMismatch",
      "description": "Transaction amount differs between systems",
      "detected_at": "2024-01-01T12:00:00Z",
      "severity": "High"
    }
  ],
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Get Anomaly
```http
GET /anomalies/{anomaly-id}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "anomaly_id": "770e8400-e29b-41d4-a716-446655440000",
    "transaction_id": "550e8400-e29b-41d4-a716-446655440000",
    "anomaly_type": "AmountMismatch",
    "description": "Transaction amount differs between systems",
    "detected_at": "2024-01-01T12:00:00Z",
    "severity": "High"
  },
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### List Daily Summaries
```http
GET /daily-summaries?limit=30&offset=0
```

**Query Parameters:**
- `limit` (optional): Number of summaries to return (default: 30, max: 365)
- `offset` (optional): Number of summaries to skip (default: 0)

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "date": "2024-01-01",
      "total_transactions": 1500,
      "total_amount": 15000000,
      "committed_count": 1200,
      "failed_count": 200,
      "pending_count": 100,
      "anomalies_count": 5
    }
  ],
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Trigger Reconciliation
```http
POST /reconcile
```

**Response:**
```json
{
  "success": true,
  "data": "Reconciliation completed. Found 3 anomalies.",
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Start Event Replay
```http
POST /replay/start
```

**Response:**
```json
{
  "success": true,
  "data": "550e8400-e29b-41d4-a716-446655440000",
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Get Replay Status
```http
GET /replay/{replay-id}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "replay_id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "RUNNING",
    "started_at": "2024-01-01T12:00:00Z",
    "completed_at": null,
    "events_processed": 1500,
    "events_total": 2000,
    "errors_count": 5,
    "error_message": null
  },
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### List Replays
```http
GET /replay?limit=50&offset=0
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "replay_id": "550e8400-e29b-41d4-a716-446655440000",
      "status": "COMPLETED",
      "started_at": "2024-01-01T12:00:00Z",
      "completed_at": "2024-01-01T12:05:00Z",
      "events_processed": 2000,
      "events_total": 2000,
      "errors_count": 0,
      "error_message": null
    }
  ],
  "error": null,
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Get Metrics (Prometheus)
```http
GET /metrics
```

**Response:**
```
# HELP transactions_total Total number of transactions processed
# TYPE transactions_total counter
transactions_total 1500

# HELP reconciliation_runs_total Total number of reconciliation runs
# TYPE reconciliation_runs_total counter
reconciliation_runs_total 25

# HELP anomalies_detected_total Total number of anomalies detected
# TYPE anomalies_detected_total counter
anomalies_detected_total 15
```

---

## Error Responses

All endpoints return consistent error responses:

```json
{
  "success": false,
  "data": null,
  "error": "Error description",
  "timestamp": "2024-01-01T12:00:00Z"
}
```

### Common HTTP Status Codes

- `200 OK`: Successful request
- `400 Bad Request`: Invalid input or business logic error
- `401 Unauthorized`: Authentication required
- `403 Forbidden`: Insufficient permissions
- `404 Not Found`: Resource not found
- `409 Conflict`: Resource conflict (e.g., duplicate external_id)
- `500 Internal Server Error`: System error

### Rate Limiting

Currently, rate limiting is not implemented but can be added using Redis-based counters. Recommended limits:

- Payment Processor: 1000 requests/minute per client
- Reconciliation Service: 100 requests/minute per client

### Pagination

All list endpoints support pagination using `limit` and `offset` parameters:

- `limit`: Maximum number of items to return
- `offset`: Number of items to skip
- Default `limit`: Varies by endpoint
- Maximum `limit`: 1000 for most endpoints

### Data Types

#### Transaction States
- `PENDING`: Transaction created, awaiting processing
- `COMMITTED`: Transaction successfully completed
- `FAILED`: Transaction failed during processing
- `CANCELLED`: Transaction cancelled by user/system

#### Anomaly Types
- `MissingTransaction`: Transaction exists in one system but not another
- `AmountMismatch`: Transaction amounts differ between systems
- `StateMismatch`: Transaction states differ between systems
- `DuplicateTransaction`: Same transaction processed multiple times
- `OrphanedEvent`: Event exists without corresponding transaction

#### Anomaly Severities
- `LOW`: Minor discrepancy, no immediate action required
- `MEDIUM`: Moderate discrepancy, investigation recommended
- `HIGH`: Significant discrepancy, immediate investigation required
- `CRITICAL`: Critical discrepancy, immediate action required




