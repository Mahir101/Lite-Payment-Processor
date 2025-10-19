# DFSP-Lite Payments - Deployment Guide

## Prerequisites

### System Requirements
- **Docker**: Version 20.10+ with Docker Compose v2
- **Memory**: Minimum 4GB RAM (8GB recommended)
- **Storage**: Minimum 10GB free space
- **CPU**: 2+ cores recommended

### Optional Tools
- **k6**: For load testing
- **PostgreSQL Client**: For database management
- **Redis CLI**: For Redis management

## Quick Start

### 1. Clone Repository
```bash
git clone <repository-url>
cd dfsp-lite-payments
```

### 2. Start All Services
```bash
docker-compose up -d
```

### 3. Verify Deployment
```bash
# Check service status
docker-compose ps

# Test Payment Processor
curl http://localhost:3001/health

# Test Reconciliation Service
curl http://localhost:3002/health
```

### 4. Run Load Tests
```bash
# Install k6 (if not already installed)
# Windows: choco install k6
# macOS: brew install k6
# Linux: apt-get install k6

# Run load tests
k6 run load-tests/load-test.js
k6 run load-tests/reconciliation-test.js
```

## Detailed Deployment

### Environment Configuration

Create a `.env` file for custom configuration:

```bash
# Database Configuration
POSTGRES_PASSWORD=your-secure-password
PAYMENT_DB_NAME=payment_processor
RECONCILIATION_DB_NAME=reconciliation

# Redis Configuration
REDIS_PASSWORD=your-redis-password

# JWT Configuration
JWT_SECRET=your-jwt-secret-key-change-in-production

# Service Configuration
PAYMENT_SERVICE_PORT=3001
RECONCILIATION_SERVICE_PORT=3002

# Logging
RUST_LOG=info
```

### Service-Specific Deployment

#### Payment Processor Only
```bash
# Start only payment processor dependencies
docker-compose up -d payment-db redis

# Build and run payment processor locally
cd payment-processor
cargo run
```

#### Reconciliation Service Only
```bash
# Start only reconciliation service dependencies
docker-compose up -d reconciliation-db redis

# Build and run reconciliation service locally
cd reconciliation-service
cargo run
```

### Database Setup

#### Manual Database Initialization
```bash
# Payment Processor Database
docker exec -it dfsp-lite-payments-payment-db-1 psql -U postgres -d payment_processor -f /docker-entrypoint-initdb.d/001_payment_processor_schema.sql

# Reconciliation Database
docker exec -it dfsp-lite-payments-reconciliation-db-1 psql -U postgres -d reconciliation -f /docker-entrypoint-initdb.d/002_reconciliation_schema.sql
```

#### Verify Database Setup
```bash
# Check Payment Processor tables
docker exec -it dfsp-lite-payments-payment-db-1 psql -U postgres -d payment_processor -c "\dt"

# Check Reconciliation tables
docker exec -it dfsp-lite-payments-reconciliation-db-1 psql -U postgres -d reconciliation -c "\dt"
```

## Production Deployment

### Security Considerations

#### 1. Change Default Passwords
```bash
# Generate secure passwords
openssl rand -base64 32  # For PostgreSQL
openssl rand -base64 32  # For Redis
openssl rand -base64 64  # For JWT secret
```

#### 2. Network Security
```yaml
# docker-compose.prod.yml
version: '3.8'
services:
  payment-processor:
    networks:
      - internal
    expose:
      - "3001"
  
  reconciliation-service:
    networks:
      - internal
    expose:
      - "3002"

networks:
  internal:
    driver: bridge
    internal: true
```

#### 3. Resource Limits
```yaml
# docker-compose.prod.yml
services:
  payment-processor:
    deploy:
      resources:
        limits:
          memory: 512M
          cpus: '0.5'
        reservations:
          memory: 256M
          cpus: '0.25'
```

### Monitoring Setup

#### 1. Prometheus Configuration
```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'payment-processor'
    static_configs:
      - targets: ['payment-processor:3001']
    metrics_path: '/metrics'
    scrape_interval: 5s

  - job_name: 'reconciliation-service'
    static_configs:
      - targets: ['reconciliation-service:3002']
    metrics_path: '/metrics'
    scrape_interval: 5s
```

#### 2. Grafana Dashboard
Access Grafana at `http://localhost:3000` with credentials:
- Username: `admin`
- Password: `admin`

### Load Balancing

#### Nginx Configuration
```nginx
# nginx.conf
upstream payment_processor {
    server payment-processor:3001;
    server payment-processor-2:3001;
}

upstream reconciliation_service {
    server reconciliation-service:3002;
    server reconciliation-service-2:3002;
}

server {
    listen 80;
    
    location /api/payments/ {
        proxy_pass http://payment_processor/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    
    location /api/reconciliation/ {
        proxy_pass http://reconciliation_service/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Kubernetes Deployment

### 1. Create Namespace
```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: dfsp-lite
```

### 2. ConfigMap
```yaml
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: dfsp-config
  namespace: dfsp-lite
data:
  DATABASE_URL: "postgresql://postgres:password@postgres-service:5432/payment_processor"
  REDIS_URL: "redis://redis-service:6379"
  JWT_SECRET: "your-jwt-secret"
  RUST_LOG: "info"
```

### 3. Secrets
```yaml
# secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: dfsp-secrets
  namespace: dfsp-lite
type: Opaque
data:
  postgres-password: <base64-encoded-password>
  redis-password: <base64-encoded-password>
```

### 4. Deployment
```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: payment-processor
  namespace: dfsp-lite
spec:
  replicas: 3
  selector:
    matchLabels:
      app: payment-processor
  template:
    metadata:
      labels:
        app: payment-processor
    spec:
      containers:
      - name: payment-processor
        image: dfsp-lite/payment-processor:latest
        ports:
        - containerPort: 3001
        envFrom:
        - configMapRef:
            name: dfsp-config
        - secretRef:
            name: dfsp-secrets
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 3001
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 3001
          initialDelaySeconds: 5
          periodSeconds: 5
```

### 5. Service
```yaml
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: payment-processor-service
  namespace: dfsp-lite
spec:
  selector:
    app: payment-processor
  ports:
  - port: 3001
    targetPort: 3001
  type: ClusterIP
```

## Troubleshooting

### Common Issues

#### 1. Service Won't Start
```bash
# Check logs
docker-compose logs payment-processor
docker-compose logs reconciliation-service

# Check service status
docker-compose ps
```

#### 2. Database Connection Issues
```bash
# Test database connectivity
docker exec -it dfsp-lite-payments-payment-db-1 pg_isready -U postgres

# Check database logs
docker-compose logs payment-db
```

#### 3. Redis Connection Issues
```bash
# Test Redis connectivity
docker exec -it dfsp-lite-payments-redis-1 redis-cli ping

# Check Redis logs
docker-compose logs redis
```

#### 4. Performance Issues
```bash
# Check resource usage
docker stats

# Monitor database performance
docker exec -it dfsp-lite-payments-payment-db-1 psql -U postgres -d payment_processor -c "SELECT * FROM pg_stat_activity;"
```

### Health Checks

#### Service Health Endpoints
```bash
# Payment Processor
curl http://localhost:3001/health | jq

# Reconciliation Service
curl http://localhost:3002/health | jq
```

#### Database Health
```bash
# Payment Processor DB
docker exec -it dfsp-lite-payments-payment-db-1 psql -U postgres -d payment_processor -c "SELECT 1;"

# Reconciliation DB
docker exec -it dfsp-lite-payments-reconciliation-db-1 psql -U postgres -d reconciliation -c "SELECT 1;"
```

### Log Analysis

#### Application Logs
```bash
# Follow logs in real-time
docker-compose logs -f payment-processor
docker-compose logs -f reconciliation-service

# Filter by log level
docker-compose logs payment-processor | grep ERROR
```

#### Database Logs
```bash
# PostgreSQL logs
docker-compose logs payment-db | grep ERROR
docker-compose logs reconciliation-db | grep ERROR
```

## Backup and Recovery

### Database Backups
```bash
# Backup Payment Processor DB
docker exec dfsp-lite-payments-payment-db-1 pg_dump -U postgres payment_processor > payment_processor_backup.sql

# Backup Reconciliation DB
docker exec dfsp-lite-payments-reconciliation-db-1 pg_dump -U postgres reconciliation > reconciliation_backup.sql
```

### Database Restore
```bash
# Restore Payment Processor DB
docker exec -i dfsp-lite-payments-payment-db-1 psql -U postgres payment_processor < payment_processor_backup.sql

# Restore Reconciliation DB
docker exec -i dfsp-lite-payments-reconciliation-db-1 psql -U postgres reconciliation < reconciliation_backup.sql
```

### Automated Backups
```bash
#!/bin/bash
# backup.sh
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p backups

# Backup databases
docker exec dfsp-lite-payments-payment-db-1 pg_dump -U postgres payment_processor > "backups/payment_processor_${DATE}.sql"
docker exec dfsp-lite-payments-reconciliation-db-1 pg_dump -U postgres reconciliation > "backups/reconciliation_${DATE}.sql"

# Compress backups
gzip "backups/payment_processor_${DATE}.sql"
gzip "backups/reconciliation_${DATE}.sql"

# Keep only last 7 days of backups
find backups -name "*.sql.gz" -mtime +7 -delete
```

## Scaling

### Horizontal Scaling
```bash
# Scale Payment Processor
docker-compose up -d --scale payment-processor=3

# Scale Reconciliation Service
docker-compose up -d --scale reconciliation-service=2
```

### Database Scaling
```yaml
# docker-compose.scale.yml
services:
  payment-db:
    deploy:
      replicas: 1
    volumes:
      - payment_db_data:/var/lib/postgresql/data
      - ./postgresql.conf:/etc/postgresql/postgresql.conf
```

### Load Testing at Scale
```bash
# Test with multiple instances
k6 run --vus 500 --duration 60s load-tests/load-test.js
```

This deployment guide provides comprehensive instructions for deploying the DFSP-Lite Payments platform in various environments, from development to production.





