//! # Invoice Service
//! 
//! This module handles invoice generation, management, and payment tracking.

use shared::{Invoice, InvoiceStatus, InvoiceLineItem, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::{Utc, Duration};

pub struct InvoiceService {
    pool: PgPool,
}

impl InvoiceService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new invoice
    pub async fn create_invoice(
        &self,
        customer_id: Uuid,
        subscription_id: Option<Uuid>,
        line_items: Vec<InvoiceLineItemInput>,
        due_date: Option<chrono::DateTime<Utc>>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Invoice, PaymentError> {
        let invoice_id = Uuid::new_v4();
        let now = Utc::now();
        
        // Calculate total amount
        let amount_due: i64 = line_items.iter()
            .map(|item| item.amount * item.quantity as i64)
            .sum();

        // Determine currency (from first line item or default)
        let currency = line_items.first()
            .map(|_| "USD".to_string())
            .unwrap_or_else(|| "USD".to_string());

        // Create invoice
        sqlx::query(
            r#"
            INSERT INTO invoices (
                id, customer_id, subscription_id, status, amount_due, amount_paid,
                currency, due_date, metadata, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#
        )
        .bind(invoice_id)
        .bind(customer_id)
        .bind(subscription_id)
        .bind(InvoiceStatus::Draft.to_string())
        .bind(amount_due)
        .bind(0i64)
        .bind(&currency)
        .bind(due_date)
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create invoice: {}", e)))?;

        // Create line items
        for item in line_items {
            let line_item_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO invoice_line_items (
                    id, invoice_id, description, amount, quantity, metadata, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#
            )
            .bind(line_item_id)
            .bind(invoice_id)
            .bind(item.description.as_deref())
            .bind(item.amount)
            .bind(item.quantity as i32)
            .bind(serde_json::to_value(item.metadata.unwrap_or_default()).unwrap())
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to create line item: {}", e)))?;
        }

        Ok(Invoice {
            id: invoice_id,
            customer_id,
            subscription_id,
            status: InvoiceStatus::Draft,
            amount_due,
            amount_paid: 0,
            currency,
            due_date,
            paid_at: None,
            invoice_pdf_url: None,
            hosted_invoice_url: Some(format!("https://example.com/invoices/{}", invoice_id)),
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Finalizes an invoice (changes from DRAFT to OPEN)
    pub async fn finalize_invoice(&self, invoice_id: Uuid) -> Result<Invoice, PaymentError> {
        sqlx::query("UPDATE invoices SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(InvoiceStatus::Open.to_string())
            .bind(Utc::now())
            .bind(invoice_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to finalize invoice: {}", e)))?;

        self.get_invoice(invoice_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Invoice not found after finalization".to_string()))
    }

    /// Marks an invoice as paid
    pub async fn mark_invoice_paid(
        &self,
        invoice_id: Uuid,
        amount_paid: i64,
    ) -> Result<Invoice, PaymentError> {
        let invoice = self.get_invoice(invoice_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Invoice not found".to_string()))?;

        let new_amount_paid = invoice.amount_paid + amount_paid;
        let new_status = if new_amount_paid >= invoice.amount_due {
            InvoiceStatus::Paid
        } else {
            invoice.status
        };

        sqlx::query(
            r#"
            UPDATE invoices
            SET status = $1, amount_paid = $2, paid_at = $3, updated_at = $4
            WHERE id = $5
            "#
        )
        .bind(new_status.to_string())
        .bind(new_amount_paid)
        .bind(Some(Utc::now()))
        .bind(Utc::now())
        .bind(invoice_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to mark invoice paid: {}", e)))?;

        self.get_invoice(invoice_id).await?
            .ok_or_else(|| PaymentError::DatabaseError("Invoice not found after update".to_string()))
    }

    /// Gets an invoice by ID
    pub async fn get_invoice(&self, invoice_id: Uuid) -> Result<Option<Invoice>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, customer_id, subscription_id, status, amount_due, amount_paid,
                   currency, due_date, paid_at, invoice_pdf_url, hosted_invoice_url,
                   metadata, created_at, updated_at
            FROM invoices
            WHERE id = $1
            "#
        )
        .bind(invoice_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get invoice: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_invoice(row)?)),
            None => Ok(None),
        }
    }

    /// Lists invoices for a customer
    pub async fn list_invoices(
        &self,
        customer_id: Option<Uuid>,
        status: Option<InvoiceStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Invoice>, PaymentError> {
        let query = match (customer_id, status) {
            (Some(cid), Some(st)) => {
                sqlx::query(
                    r#"
                    SELECT id, customer_id, subscription_id, status, amount_due, amount_paid,
                           currency, due_date, paid_at, invoice_pdf_url, hosted_invoice_url,
                           metadata, created_at, updated_at
                    FROM invoices
                    WHERE customer_id = $1 AND status = $2
                    ORDER BY created_at DESC
                    LIMIT $3 OFFSET $4
                    "#
                )
                .bind(cid)
                .bind(st.to_string())
                .bind(limit)
                .bind(offset)
            }
            (Some(cid), None) => {
                sqlx::query(
                    r#"
                    SELECT id, customer_id, subscription_id, status, amount_due, amount_paid,
                           currency, due_date, paid_at, invoice_pdf_url, hosted_invoice_url,
                           metadata, created_at, updated_at
                    FROM invoices
                    WHERE customer_id = $1
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#
                )
                .bind(cid)
                .bind(limit)
                .bind(offset)
            }
            (None, Some(st)) => {
                sqlx::query(
                    r#"
                    SELECT id, customer_id, subscription_id, status, amount_due, amount_paid,
                           currency, due_date, paid_at, invoice_pdf_url, hosted_invoice_url,
                           metadata, created_at, updated_at
                    FROM invoices
                    WHERE status = $1
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#
                )
                .bind(st.to_string())
                .bind(limit)
                .bind(offset)
            }
            (None, None) => {
                sqlx::query(
                    r#"
                    SELECT id, customer_id, subscription_id, status, amount_due, amount_paid,
                           currency, due_date, paid_at, invoice_pdf_url, hosted_invoice_url,
                           metadata, created_at, updated_at
                    FROM invoices
                    ORDER BY created_at DESC
                    LIMIT $1 OFFSET $2
                    "#
                )
                .bind(limit)
                .bind(offset)
            }
        }
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list invoices: {}", e)))?;

        let mut invoices = Vec::new();
        for row in query {
            invoices.push(Self::row_to_invoice(row)?);
        }

        Ok(invoices)
    }

    /// Gets line items for an invoice
    pub async fn get_invoice_line_items(
        &self,
        invoice_id: Uuid,
    ) -> Result<Vec<InvoiceLineItem>, PaymentError> {
        let rows = sqlx::query(
            "SELECT id, invoice_id, description, amount, quantity, metadata, created_at FROM invoice_line_items WHERE invoice_id = $1 ORDER BY created_at"
        )
        .bind(invoice_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get line items: {}", e)))?;

        let mut items = Vec::new();
        for row in rows {
            let metadata_value: serde_json::Value = row.get("metadata");
            let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
                .unwrap_or_default();

            items.push(InvoiceLineItem {
                id: row.get("id"),
                invoice_id: row.get("invoice_id"),
                description: row.get("description"),
                amount: row.get("amount"),
                quantity: row.get::<i32, _>("quantity") as u32,
                metadata,
                created_at: row.get("created_at"),
            });
        }

        Ok(items)
    }

    /// Converts database row to Invoice
    fn row_to_invoice(row: sqlx::postgres::PgRow) -> Result<Invoice, PaymentError> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "DRAFT" => InvoiceStatus::Draft,
            "OPEN" => InvoiceStatus::Open,
            "PAID" => InvoiceStatus::Paid,
            "UNCOLLECTIBLE" => InvoiceStatus::Uncollectible,
            "VOID" => InvoiceStatus::Void,
            _ => return Err(PaymentError::DatabaseError("Invalid invoice status".to_string())),
        };

        let metadata_value: serde_json::Value = row.get("metadata");
        let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
            .unwrap_or_default();

        Ok(Invoice {
            id: row.get("id"),
            customer_id: row.get("customer_id"),
            subscription_id: row.get("subscription_id"),
            status,
            amount_due: row.get("amount_due"),
            amount_paid: row.get("amount_paid"),
            currency: row.get("currency"),
            due_date: row.get("due_date"),
            paid_at: row.get("paid_at"),
            invoice_pdf_url: row.get("invoice_pdf_url"),
            hosted_invoice_url: row.get("hosted_invoice_url"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}

#[derive(Clone)]
pub struct InvoiceLineItemInput {
    pub description: Option<String>,
    pub amount: i64,
    pub quantity: u32,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

impl std::fmt::Display for InvoiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvoiceStatus::Draft => write!(f, "DRAFT"),
            InvoiceStatus::Open => write!(f, "OPEN"),
            InvoiceStatus::Paid => write!(f, "PAID"),
            InvoiceStatus::Uncollectible => write!(f, "UNCOLLECTIBLE"),
            InvoiceStatus::Void => write!(f, "VOID"),
        }
    }
}

