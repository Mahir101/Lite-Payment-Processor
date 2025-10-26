use anyhow::Result;
use chrono::{DateTime, Utc};
use shared::{
    Anomaly, AnomalySeverity, AnomalyType, ReconciliationReport,
};
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
pub struct ReconciliationService;

impl ReconciliationService {
    pub fn new() -> Self {
        Self
    }

    pub async fn generate_report(
        &self,
        db: &crate::database::DatabaseService,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<ReconciliationReport, Box<dyn std::error::Error + Send + Sync>> {
        let report_id = Uuid::new_v4();
        
        // This is a simplified reconciliation process
        // In a real implementation, you would:
        // 1. Query the payment processor database for transactions in the period
        // 2. Query the event ledger for events in the period
        // 3. Compare and identify discrepancies
        // 4. Generate anomalies for any mismatches

        let report = ReconciliationReport {
            report_id,
            generated_at: Utc::now(),
            period_start,
            period_end,
            total_transactions: 0, // Would be calculated from actual data
            total_amount: 0,       // Would be calculated from actual data
            anomalies: Vec::new(), // Would be populated with actual anomalies
        };

        // Store the report in the database
        self.store_report(db, &report).await?;

        Ok(report)
    }

    pub async fn run_reconciliation(
        &self,
        db: &crate::database::DatabaseService,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting reconciliation process");

        // This is a simplified reconciliation
        // In a real implementation, you would:
        // 1. Get all transactions from the payment processor
        // 2. Get all events from the event ledger
        // 3. Compare totals and identify discrepancies
        // 4. Create anomaly records for any issues found

        let anomalies = self.detect_anomalies(db).await?;
        let anomaly_count = anomalies.len();
        
        for anomaly in anomalies {
            self.store_anomaly(db, &anomaly).await?;
        }

        Ok(format!("Reconciliation completed. Found {} anomalies.", anomaly_count))
    }

    async fn detect_anomalies(
        &self,
        db: &crate::database::DatabaseService,
    ) -> Result<Vec<Anomaly>, Box<dyn std::error::Error + Send + Sync>> {
        let mut anomalies = Vec::new();

        // Get event counts from event ledger
        let event_ledger_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_ledger")
            .fetch_one(&db.pool)
            .await?;

        // Get unique transaction count from events
        let unique_transaction_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT transaction_id) FROM event_ledger"
        )
        .fetch_one(&db.pool)
        .await?;

        // Check for events without proper transaction references
        let invalid_events: Vec<(Uuid, Uuid)> = sqlx::query_as(
            r#"
            SELECT id, transaction_id
            FROM event_ledger
            WHERE transaction_id IS NULL OR transaction_id = '00000000-0000-0000-0000-000000000000'
            "#
        )
        .fetch_all(&db.pool)
        .await?;

        for (event_id, transaction_id) in invalid_events {
            anomalies.push(Anomaly {
                anomaly_id: Uuid::new_v4(),
                transaction_id: Some(transaction_id),
                anomaly_type: AnomalyType::OrphanedEvent,
                description: format!("Event {} has invalid transaction reference {}", event_id, transaction_id),
                detected_at: Utc::now(),
                severity: AnomalySeverity::Medium,
            });
        }

        // Check for duplicate events
        let duplicate_events: Vec<(Uuid, i64)> = sqlx::query_as(
            r#"
            SELECT event_id, COUNT(*) as count
            FROM event_ledger
            GROUP BY event_id
            HAVING COUNT(*) > 1
            "#
        )
        .fetch_all(&db.pool)
        .await?;

        for (event_id, count) in duplicate_events {
            anomalies.push(Anomaly {
                anomaly_id: Uuid::new_v4(),
                transaction_id: None,
                anomaly_type: AnomalyType::DuplicateTransaction,
                description: format!("Event {} appears {} times in ledger", event_id, count),
                detected_at: Utc::now(),
                severity: AnomalySeverity::High,
            });
        }

        // Record anomaly metrics
        for anomaly in &anomalies {
            crate::metrics::increment_anomaly_detected(
                &format!("{:?}", anomaly.anomaly_type),
                &format!("{:?}", anomaly.severity),
            );
        }

        Ok(anomalies)
    }

    async fn store_report(
        &self,
        db: &crate::database::DatabaseService,
        report: &ReconciliationReport,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        
        sqlx::query(
            r#"
            INSERT INTO reconciliation_reports (
                report_id, generated_at, period_start, period_end,
                total_transactions, total_amount, anomalies_count,
                report_data, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (report_id) DO UPDATE SET
                generated_at = EXCLUDED.generated_at,
                period_start = EXCLUDED.period_start,
                period_end = EXCLUDED.period_end,
                total_transactions = EXCLUDED.total_transactions,
                total_amount = EXCLUDED.total_amount,
                anomalies_count = EXCLUDED.anomalies_count,
                report_data = EXCLUDED.report_data,
                updated_at = NOW()
            "#,
        )
        .bind(&report.report_id)
        .bind(&report.generated_at)
        .bind(&report.period_start)
        .bind(&report.period_end)
        .bind(&report.total_transactions)
        .bind(&report.total_amount)
        .bind(report.anomalies.len() as i64)
        .bind(&serde_json::to_string(&report.anomalies)?)
        .bind(&report.generated_at)
        .execute(&db.pool)
        .await?;

        // Record metrics
        let duration = start.elapsed().as_secs_f64();
        crate::metrics::increment_report_generated();
        crate::metrics::record_report_generation_duration(duration);
        crate::metrics::record_database_duration("store_report", duration);

        Ok(())
    }

    async fn store_anomaly(
        &self,
        db: &crate::database::DatabaseService,
        anomaly: &Anomaly,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        
        sqlx::query(
            r#"
            INSERT INTO anomalies (
                anomaly_id, transaction_id, anomaly_type, description,
                detected_at, severity, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (anomaly_id) DO UPDATE SET
                description = EXCLUDED.description,
                detected_at = EXCLUDED.detected_at,
                severity = EXCLUDED.severity,
                updated_at = NOW()
            "#,
        )
        .bind(&anomaly.anomaly_id)
        .bind(&anomaly.transaction_id)
        .bind(&serde_json::to_string(&anomaly.anomaly_type)?)
        .bind(&anomaly.description)
        .bind(&anomaly.detected_at)
        .bind(&serde_json::to_string(&anomaly.severity)?)
        .bind(&anomaly.detected_at)
        .execute(&db.pool)
        .await?;

        // Record metrics
        let duration = start.elapsed().as_secs_f64();
        crate::metrics::record_database_duration("store_anomaly", duration);

        Ok(())
    }
}



