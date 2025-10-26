use anyhow::Result;
use chrono::{DateTime, Utc};
use shared::{
    Anomaly, AnomalySeverity, AnomalyType, ReconciliationReport, TransactionEvent,
    TransactionEventType,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct DatabaseService {
    pub pool: PgPool,
    pub staging_pool: PgPool,
}

impl DatabaseService {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/reconciliation".to_string());
        
        let staging_database_url = std::env::var("STAGING_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/reconciliation_staging".to_string());

        let pool = PgPool::connect(&database_url).await?;
        let staging_pool = PgPool::connect(&staging_database_url).await?;
        
        Ok(Self { pool, staging_pool })
    }

    pub async fn health_check(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn staging_health_check(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query("SELECT 1")
            .execute(&self.staging_pool)
            .await?;
        Ok(())
    }

    pub async fn store_event(&self, event: &TransactionEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        sqlx::query(
            r#"
            INSERT INTO event_ledger (event_id, transaction_id, event_type, event_data, processed_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (event_id) DO NOTHING
            "#,
        )
        .bind(&event.event_id)
        .bind(&event.transaction_id)
        .bind(&serde_json::to_string(&event.event_type)?)
        .bind(&event.data)
        .bind(&event.timestamp)
        .bind(&event.timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_daily_summary(&self, event: &TransactionEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let today = Utc::now().date_naive();
        
        // This is a simplified version - in reality, you'd want to aggregate properly
        sqlx::query(
            r#"
            INSERT INTO daily_summaries (date, total_transactions, total_amount, committed_count, failed_count, pending_count)
            VALUES ($1, 1, 0, 0, 0, 1)
            ON CONFLICT (date) DO UPDATE SET
                total_transactions = daily_summaries.total_transactions + 1,
                pending_count = daily_summaries.pending_count + 1,
                updated_at = NOW()
            "#,
        )
        .bind(today)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_reports(&self, limit: i64, offset: i64) -> Result<Vec<ReconciliationReport>, Box<dyn std::error::Error + Send + Sync>> {
        let rows = sqlx::query(
            r#"
            SELECT report_id, generated_at, period_start, period_end, 
                   total_transactions, total_amount, anomalies_count, status
            FROM reconciliation_reports
            ORDER BY generated_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut reports = Vec::new();
        for row in rows {
            reports.push(ReconciliationReport {
                report_id: row.get("report_id"),
                generated_at: row.get("generated_at"),
                period_start: row.get("period_start"),
                period_end: row.get("period_end"),
                total_transactions: row.get("total_transactions"),
                total_amount: row.get("total_amount"),
                anomalies: Vec::new(), // Would be populated separately
            });
        }

        Ok(reports)
    }

    pub async fn get_report(&self, id: Uuid) -> Result<Option<ReconciliationReport>, Box<dyn std::error::Error + Send + Sync>> {
        let row = sqlx::query(
            r#"
            SELECT report_id, generated_at, period_start, period_end, 
                   total_transactions, total_amount, anomalies_count, status
            FROM reconciliation_reports
            WHERE report_id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(ReconciliationReport {
                report_id: row.get("report_id"),
                generated_at: row.get("generated_at"),
                period_start: row.get("period_start"),
                period_end: row.get("period_end"),
                total_transactions: row.get("total_transactions"),
                total_amount: row.get("total_amount"),
                anomalies: Vec::new(), // Would be populated separately
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn generate_csv_report(&self, report_id: Uuid) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut csv_data = String::new();
        csv_data.push_str("Transaction ID,Event Type,Timestamp,Amount,Status\n");

        let rows = sqlx::query(
            r#"
            SELECT el.transaction_id, el.event_type, el.processed_at, el.event_data
            FROM event_ledger el
            JOIN reconciliation_reports rr ON el.processed_at >= rr.period_start AND el.processed_at <= rr.period_end
            WHERE rr.report_id = $1
            ORDER BY el.processed_at
            "#,
        )
        .bind(report_id)
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            let transaction_id: Uuid = row.get("transaction_id");
            let event_type: String = row.get("event_type");
            let processed_at: DateTime<Utc> = row.get("processed_at");
            let event_data: serde_json::Value = row.get("event_data");

            csv_data.push_str(&format!(
                "{},{},{},,\n",
                transaction_id,
                event_type,
                processed_at.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        Ok(csv_data)
    }

    pub async fn list_anomalies(
        &self,
        severity_filter: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Anomaly>, Box<dyn std::error::Error + Send + Sync>> {
        let query = if let Some(severity) = severity_filter {
            sqlx::query(
                r#"
                SELECT anomaly_id, transaction_id, anomaly_type, description, 
                       severity, detected_at, resolved_at, resolution_notes
                FROM anomalies
                WHERE severity = $1
                ORDER BY detected_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(severity)
            .bind(limit)
            .bind(offset)
        } else {
            sqlx::query(
                r#"
                SELECT anomaly_id, transaction_id, anomaly_type, description, 
                       severity, detected_at, resolved_at, resolution_notes
                FROM anomalies
                ORDER BY detected_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
        };

        let rows = query.fetch_all(&self.pool).await?;

        let mut anomalies = Vec::new();
        for row in rows {
            anomalies.push(Anomaly {
                anomaly_id: row.get("anomaly_id"),
                transaction_id: row.get("transaction_id"),
                anomaly_type: self.parse_anomaly_type(row.get("anomaly_type")),
                description: row.get("description"),
                detected_at: row.get("detected_at"),
                severity: self.parse_anomaly_severity(row.get("severity")),
            });
        }

        Ok(anomalies)
    }

    pub async fn get_anomaly(&self, id: Uuid) -> Result<Option<Anomaly>, Box<dyn std::error::Error + Send + Sync>> {
        let row = sqlx::query(
            r#"
            SELECT anomaly_id, transaction_id, anomaly_type, description, 
                   severity, detected_at, resolved_at, resolution_notes
            FROM anomalies
            WHERE anomaly_id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Anomaly {
                anomaly_id: row.get("anomaly_id"),
                transaction_id: row.get("transaction_id"),
                anomaly_type: self.parse_anomaly_type(row.get("anomaly_type")),
                description: row.get("description"),
                detected_at: row.get("detected_at"),
                severity: self.parse_anomaly_severity(row.get("severity")),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_daily_summaries(&self, limit: i64, offset: i64) -> Result<Vec<DailySummary>, Box<dyn std::error::Error + Send + Sync>> {
        let rows = sqlx::query(
            r#"
            SELECT date, total_transactions, total_amount, committed_count, 
                   failed_count, pending_count, anomalies_count
            FROM daily_summaries
            ORDER BY date DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(DailySummary {
                date: row.get("date"),
                total_transactions: row.get("total_transactions"),
                total_amount: row.get("total_amount"),
                committed_count: row.get("committed_count"),
                failed_count: row.get("failed_count"),
                pending_count: row.get("pending_count"),
                anomalies_count: row.get("anomalies_count"),
            });
        }

        Ok(summaries)
    }

    fn parse_anomaly_type(&self, type_str: String) -> AnomalyType {
        match type_str.as_str() {
            "MissingTransaction" => AnomalyType::MissingTransaction,
            "AmountMismatch" => AnomalyType::AmountMismatch,
            "StateMismatch" => AnomalyType::StateMismatch,
            "DuplicateTransaction" => AnomalyType::DuplicateTransaction,
            "OrphanedEvent" => AnomalyType::OrphanedEvent,
            _ => AnomalyType::MissingTransaction,
        }
    }

    fn parse_anomaly_severity(&self, severity_str: String) -> AnomalySeverity {
        match severity_str.as_str() {
            "LOW" => AnomalySeverity::Low,
            "MEDIUM" => AnomalySeverity::Medium,
            "HIGH" => AnomalySeverity::High,
            "CRITICAL" => AnomalySeverity::Critical,
            _ => AnomalySeverity::Low,
        }
    }
}

#[derive(serde::Serialize)]
pub struct DailySummary {
    pub date: chrono::NaiveDate,
    pub total_transactions: i64,
    pub total_amount: i64,
    pub committed_count: i64,
    pub failed_count: i64,
    pub pending_count: i64,
    pub anomalies_count: i32,
}



