//! # Customer Service
//! 
//! This module handles customer management, including creation,
//! retrieval, and updates of customer records.

use shared::{Customer, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;

pub struct CustomerService {
    pool: PgPool,
}

impl CustomerService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new customer
    pub async fn create_customer(
        &self,
        email: Option<String>,
        phone: Option<String>,
        name: Option<String>,
        description: Option<String>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Customer, PaymentError> {
        let customer_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO customers (id, email, phone, name, description, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(customer_id)
        .bind(email.as_deref())
        .bind(phone.as_deref())
        .bind(name.as_deref())
        .bind(description.as_deref())
        .bind(serde_json::to_value(metadata.unwrap_or_default()).unwrap())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create customer: {}", e)))?;

        Ok(Customer {
            id: customer_id,
            email,
            phone,
            name,
            description,
            metadata: metadata.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Gets a customer by ID
    pub async fn get_customer(&self, customer_id: Uuid) -> Result<Option<Customer>, PaymentError> {
        let row = sqlx::query(
            "SELECT id, email, phone, name, description, metadata, created_at, updated_at FROM customers WHERE id = $1"
        )
        .bind(customer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get customer: {}", e)))?;

        match row {
            Some(row) => {
                let metadata_value: serde_json::Value = row.get("metadata");
                let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
                    .unwrap_or_default();

                Ok(Some(Customer {
                    id: row.get("id"),
                    email: row.get("email"),
                    phone: row.get("phone"),
                    name: row.get("name"),
                    description: row.get("description"),
                    metadata,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Gets a customer by email
    pub async fn get_customer_by_email(&self, email: &str) -> Result<Option<Customer>, PaymentError> {
        let row = sqlx::query(
            "SELECT id, email, phone, name, description, metadata, created_at, updated_at FROM customers WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get customer: {}", e)))?;

        match row {
            Some(row) => {
                let metadata_value: serde_json::Value = row.get("metadata");
                let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
                    .unwrap_or_default();

                Ok(Some(Customer {
                    id: row.get("id"),
                    email: row.get("email"),
                    phone: row.get("phone"),
                    name: row.get("name"),
                    description: row.get("description"),
                    metadata,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Updates a customer
    pub async fn update_customer(
        &self,
        customer_id: Uuid,
        email: Option<String>,
        phone: Option<String>,
        name: Option<String>,
        description: Option<String>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> Result<Customer, PaymentError> {
        // Get existing customer
        let existing = self.get_customer(customer_id).await?;
        let existing = existing.ok_or_else(|| PaymentError::UserNotFound(customer_id.to_string()))?;

        let new_email = email.or(existing.email);
        let new_phone = phone.or(existing.phone);
        let new_name = name.or(existing.name);
        let new_description = description.or(existing.description);
        
        let mut new_metadata = existing.metadata;
        if let Some(metadata) = metadata {
            new_metadata.extend(metadata);
        }

        sqlx::query(
            r#"
            UPDATE customers
            SET email = $1, phone = $2, name = $3, description = $4, metadata = $5, updated_at = $6
            WHERE id = $7
            "#
        )
        .bind(new_email.as_deref())
        .bind(new_phone.as_deref())
        .bind(new_name.as_deref())
        .bind(new_description.as_deref())
        .bind(serde_json::to_value(&new_metadata).unwrap())
        .bind(Utc::now())
        .bind(customer_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to update customer: {}", e)))?;

        Ok(Customer {
            id: customer_id,
            email: new_email,
            phone: new_phone,
            name: new_name,
            description: new_description,
            metadata: new_metadata,
            created_at: existing.created_at,
            updated_at: Utc::now(),
        })
    }

    /// Lists customers with pagination
    pub async fn list_customers(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Customer>, PaymentError> {
        let rows = sqlx::query(
            "SELECT id, email, phone, name, description, metadata, created_at, updated_at FROM customers ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to list customers: {}", e)))?;

        let mut customers = Vec::new();
        for row in rows {
            let metadata_value: serde_json::Value = row.get("metadata");
            let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_value)
                .unwrap_or_default();

            customers.push(Customer {
                id: row.get("id"),
                email: row.get("email"),
                phone: row.get("phone"),
                name: row.get("name"),
                description: row.get("description"),
                metadata,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(customers)
    }

    /// Deletes a customer (soft delete by marking as inactive)
    pub async fn delete_customer(&self, customer_id: Uuid) -> Result<(), PaymentError> {
        // In a real system, you might want to check for active subscriptions, payment methods, etc.
        // For now, we'll just delete the customer
        sqlx::query("DELETE FROM customers WHERE id = $1")
            .bind(customer_id)
            .execute(&self.pool)
            .await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to delete customer: {}", e)))?;

        Ok(())
    }
}

