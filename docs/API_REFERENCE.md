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

## Refund Endpoints

### POST /transactions/{id}/refund
**Description**: Creates a refund for a transaction (full or partial)

**Path Parameters**:
- `id`: Transaction UUID

**Request Body**:
```json
{
  "amount": 5000,
  "reason": "requested_by_customer",
  "metadata": {
    "reason_note": "Customer requested refund"
  }
}
```

**Response**:
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "transaction_id": "550e8400-e29b-41d4-a716-446655440000",
    "amount": 5000,
    "status": "PENDING",
    "reason": "RequestedByCustomer",
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

### GET /refunds/{id}
**Description**: Retrieves a refund by ID

**Path Parameters**:
- `id`: Refund UUID

### GET /transactions/{id}/refunds
**Description**: Lists all refunds for a transaction

**Path Parameters**:
- `id`: Transaction UUID

---

## Customer Endpoints

### POST /customers
**Description**: Creates a new customer

**Request Body**:
```json
{
  "email": "customer@example.com",
  "phone": "+1234567890",
  "name": "John Doe",
  "description": "Premium customer",
  "metadata": {
    "loyalty_tier": "gold"
  }
}
```

### GET /customers/{id}
**Description**: Retrieves a customer by ID

### PUT /customers/{id}
**Description**: Updates customer information

### GET /customers
**Description**: Lists customers with pagination

**Query Parameters**:
- `limit`: Maximum results (default: 100, max: 1000)
- `offset`: Skip results (default: 0)

### DELETE /customers/{id}
**Description**: Deletes a customer

---

## Payment Method Endpoints

### POST /payment-methods
**Description**: Creates a tokenized payment method (PCI compliant)

**Request Body**:
```json
{
  "customer_id": "550e8400-e29b-41d4-a716-446655440000",
  "type": "card",
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
  "is_default": true
}
```

### GET /payment-methods/{id}
**Description**: Retrieves a payment method by ID

### GET /payment-methods
**Description**: Lists payment methods (optionally filtered by customer)

**Query Parameters**:
- `customer_id`: Filter by customer UUID

### DELETE /payment-methods/{id}
**Description**: Deletes a payment method

### POST /customers/{customer_id}/payment-methods/{id}/default
**Description**: Sets a payment method as default for a customer

---

## Webhook Endpoints

### POST /webhooks
**Description**: Creates a webhook endpoint

**Request Body**:
```json
{
  "url": "https://example.com/webhook",
  "events": ["transaction.created", "refund.created"],
  "metadata": {}
}
```

### GET /webhooks/{id}
**Description**: Retrieves a webhook configuration

### GET /webhooks
**Description**: Lists all webhook configurations

### POST /webhook
**Description**: Receives webhook delivery (for testing)

---

## Subscription Endpoints

### POST /subscriptions
**Description**: Creates a subscription for recurring billing

**Request Body**:
```json
{
  "customer_id": "550e8400-e29b-41d4-a716-446655440000",
  "price_id": "550e8400-e29b-41d4-a716-446655440000",
  "trial_days": 14,
  "metadata": {}
}
```

### GET /subscriptions/{id}
**Description**: Retrieves a subscription by ID

### GET /subscriptions
**Description**: Lists subscriptions (optionally filtered by customer/status)

**Query Parameters**:
- `customer_id`: Filter by customer UUID
- `status`: Filter by status (INCOMPLETE, ACTIVE, CANCELED, TRIALING)

### POST /subscriptions/{id}/cancel
**Description**: Cancels a subscription

**Request Body**:
```json
{
  "at_period_end": false
}
```

---

## Dispute Endpoints

### POST /disputes
**Description**: Creates a dispute/chargeback record

**Request Body**:
```json
{
  "transaction_id": "550e8400-e29b-41d4-a716-446655440000",
  "reason": "FRAUDULENT",
  "metadata": {}
}
```

### GET /disputes/{id}
**Description**: Retrieves a dispute by ID

### GET /disputes
**Description**: Lists disputes with optional status filter

**Query Parameters**:
- `status`: Filter by status (NEEDS_RESPONSE, UNDER_REVIEW, WON, LOST)
- `limit`: Maximum results (default: 100)
- `offset`: Skip results (default: 0)

### POST /disputes/{id}/submit-evidence
**Description**: Submits evidence for a dispute

### POST /disputes/{id}/update-status
**Description**: Updates dispute status (WON, LOST, CHARGE_REFUNDED)

---

## Invoice Endpoints

### POST /invoices
**Description**: Creates an invoice

**Request Body**:
```json
{
  "customer_id": "550e8400-e29b-41d4-a716-446655440000",
  "subscription_id": "550e8400-e29b-41d4-a716-446655440000",
  "line_items": [
    {
      "description": "Monthly subscription",
      "amount": 10000,
      "quantity": 1
    }
  ],
  "due_date": "2024-02-01T00:00:00Z"
}
```

### GET /invoices/{id}
**Description**: Retrieves an invoice by ID

### GET /invoices
**Description**: Lists invoices with optional filters

**Query Parameters**:
- `customer_id`: Filter by customer UUID
- `status`: Filter by status (DRAFT, OPEN, PAID)
- `limit`: Maximum results (default: 100)
- `offset`: Skip results (default: 0)

### POST /invoices/{id}/finalize
**Description**: Finalizes a draft invoice

### POST /invoices/{id}/pay
**Description**: Marks an invoice as paid

### GET /invoices/{id}/line-items
**Description**: Retrieves line items for an invoice

---

## Payout Endpoints

### POST /payouts
**Description**: Creates a payout

**Request Body**:
```json
{
  "account_id": "550e8400-e29b-41d4-a716-446655440000",
  "amount": 50000,
  "currency": "USD",
  "payout_method": "bank_account",
  "metadata": {}
}
```

### GET /payouts/{id}
**Description**: Retrieves a payout by ID

### GET /payouts
**Description**: Lists payouts with optional filters

**Query Parameters**:
- `account_id`: Filter by account UUID
- `status`: Filter by status (PENDING, PAID, FAILED)
- `limit`: Maximum results (default: 100)
- `offset`: Skip results (default: 0)

### POST /payouts/{id}/cancel
**Description**: Cancels a pending payout

---

## Marketplace/Connect Endpoints

### POST /connect/accounts
**Description**: Creates a Connect account for marketplace

**Request Body**:
```json
{
  "email": "merchant@example.com",
  "country": "US",
  "account_type": "express",
  "metadata": {}
}
```

### GET /connect/accounts/{id}
**Description**: Retrieves a Connect account by ID

### POST /connect/accounts/{id}/update
**Description**: Updates Connect account status

**Request Body**:
```json
{
  "charges_enabled": true,
  "payouts_enabled": true,
  "details_submitted": true
}
```

### POST /transfers
**Description**: Creates a transfer to a Connect account

**Request Body**:
```json
{
  "transaction_id": "550e8400-e29b-41d4-a716-446655440000",
  "destination_account_id": "550e8400-e29b-41d4-a716-446655440000",
  "amount": 10000,
  "currency": "USD"
}
```

### GET /transactions/{id}/transfers
**Description**: Lists transfers for a transaction

---

## Payment Intent Endpoints

### POST /payment-intents
**Description**: Creates a Payment Intent (two-step payment confirmation)

**Request Body**:
```json
{
  "customer_id": "550e8400-e29b-41d4-a716-446655440000",
  "payment_method_id": "550e8400-e29b-41d4-a716-446655440000",
  "amount": 10000,
  "currency": "USD",
  "confirmation_method": "automatic",
  "metadata": {}
}
```

### GET /payment-intents/{id}
**Description**: Retrieves a Payment Intent by ID

### POST /payment-intents/{id}/confirm
**Description**: Confirms a Payment Intent

**Request Body**:
```json
{
  "payment_method_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

### POST /payment-intents/{id}/cancel
**Description**: Cancels a Payment Intent

### POST /payment-intents/{id}/3d-secure
**Description**: Handles 3D Secure authentication result

**Request Body**:
```json
{
  "authentication_result": true
}
```

---

## Currency & Exchange Rate Endpoints

### POST /currency/convert
**Description**: Converts amount between currencies

**Request Body**:
```json
{
  "amount": 10000,
  "from_currency": "USD",
  "to_currency": "EUR"
}
```

### GET /currency/exchange-rates
**Description**: Gets exchange rate between currencies

**Query Parameters**:
- `base_currency`: Base currency code
- `target_currency`: Target currency code

### POST /currency/exchange-rates
**Description**: Sets or updates exchange rate

**Request Body**:
```json
{
  "base_currency": "USD",
  "target_currency": "EUR",
  "rate": 0.92
}
```

### GET /currency/supported
**Description**: Lists all supported currencies

---

## Tax Calculation Endpoints

### POST /tax/calculate
**Description**: Calculates tax for an amount

**Request Body**:
```json
{
  "amount": 10000,
  "country": "US",
  "jurisdiction": "NY"
}
```

### POST /tax/rates
**Description**: Creates a tax rate

**Request Body**:
```json
{
  "display_name": "Sales Tax",
  "percentage": 8.5,
  "inclusive": false,
  "country": "US",
  "jurisdiction": "NY"
}
```

### GET /tax/rates
**Description**: Lists tax rates

**Query Parameters**:
- `country`: Filter by country
- `active_only`: Show only active rates (default: true)

---

## Test Mode Endpoints

### GET /test-mode/status
**Description**: Gets test mode status

**Response**:
```json
{
  "success": true,
  "data": {
    "test_mode_enabled": true
  }
}
```

### POST /test-mode/enable
**Description**: Enables or disables test mode

**Request Body**:
```json
{
  "enabled": true
}
```

### GET /test-mode/cards
**Description**: Gets test card numbers for testing

**Response**:
```json
{
  "success": true,
  "data": {
    "success": ["4242424242424242"],
    "decline": ["4000000000000002"],
    "insufficient_funds": ["4000000000009995"]
  }
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

Rate limiting is now implemented using PostgreSQL-based tracking. Limits are enforced per API key and endpoint.

### Rate Limit Headers
All responses include rate limit headers:
- `X-RateLimit-Remaining`: Number of requests remaining in the current window
- `X-RateLimit-Reset`: Unix timestamp when the rate limit window resets

### Rate Limit Defaults
- Default limit: 100 requests per 60 seconds per API key
- Rate limit window: 60 seconds (configurable)
- Endpoint-specific limits can be configured

### Rate Limit Exceeded Response
When rate limit is exceeded, the API returns:
- Status Code: `429 Too Many Requests`
- Response Body:
```json
{
  "error": "Rate limit exceeded",
  "remaining": 0,
  "reset_at": "2024-01-15T10:31:00Z"
}
```

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
