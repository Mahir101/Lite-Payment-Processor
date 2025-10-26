-- Create multiple databases in a single PostgreSQL instance
-- This script runs after the main database is created

-- Create reconciliation database if it doesn't exist
SELECT 'CREATE DATABASE reconciliation'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'reconciliation')\gexec

-- Create reconciliation staging database if it doesn't exist
SELECT 'CREATE DATABASE reconciliation_staging'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'reconciliation_staging')\gexec

-- Create monitoring database if it doesn't exist
SELECT 'CREATE DATABASE monitoring'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'monitoring')\gexec

-- Grant permissions to postgres user for all databases
GRANT ALL PRIVILEGES ON DATABASE reconciliation TO postgres;
GRANT ALL PRIVILEGES ON DATABASE reconciliation_staging TO postgres;
GRANT ALL PRIVILEGES ON DATABASE monitoring TO postgres;
