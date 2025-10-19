use shared::{UserInfo, Account, AccountType, PaymentError};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;

#[derive(Clone)]
pub struct UserService {
    pool: PgPool,
}

impl UserService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates a new user
    pub async fn create_user(&self, email: &str, phone: Option<&str>, device_id: Option<&str>) -> Result<UserInfo, PaymentError> {
        let user_id = Uuid::new_v4();
        
        // Check if user already exists
        if let Ok(Some(_)) = self.get_user_by_email(email).await {
            return Err(PaymentError::InvalidAccount("User already exists".to_string()));
        }

        let user = UserInfo {
            id: user_id,
            email: email.to_string(),
            phone: phone.map(|s| s.to_string()),
            device_id: device_id.map(|s| s.to_string()),
            created_at: Utc::now(),
            is_verified: false, // Will be verified through email/phone verification
        };

        // Insert user into database
        sqlx::query(
            r#"
            INSERT INTO users (id, email, phone, device_id, is_verified, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(&user.id)
        .bind(&user.email)
        .bind(&user.phone)
        .bind(&user.device_id)
        .bind(user.is_verified)
        .bind(user.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create user: {}", e)))?;

        Ok(user)
    }

    /// Gets user by email
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UserInfo>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, phone, device_id, is_verified, created_at
            FROM users
            WHERE email = $1
            "#
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get user: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(UserInfo {
                id: row.get("id"),
                email: row.get("email"),
                phone: row.get("phone"),
                device_id: row.get("device_id"),
                is_verified: row.get("is_verified"),
                created_at: row.get("created_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Gets user by ID
    pub async fn get_user_by_id(&self, user_id: Uuid) -> Result<Option<UserInfo>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, phone, device_id, is_verified, created_at
            FROM users
            WHERE id = $1
            "#
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get user: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(UserInfo {
                id: row.get("id"),
                email: row.get("email"),
                phone: row.get("phone"),
                device_id: row.get("device_id"),
                is_verified: row.get("is_verified"),
                created_at: row.get("created_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Verifies a user (simulates email/phone verification)
    pub async fn verify_user(&self, user_id: Uuid) -> Result<(), PaymentError> {
        sqlx::query(
            r#"
            UPDATE users
            SET is_verified = true
            WHERE id = $1
            "#
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to verify user: {}", e)))?;

        Ok(())
    }

    /// Creates a new account for a user
    pub async fn create_account(
        &self,
        user_id: Uuid,
        account_type: AccountType,
        currency: &str,
        initial_balance: i64,
    ) -> Result<Account, PaymentError> {
        let account_id = Uuid::new_v4();
        let account_number = self.generate_account_number();

        let account = Account {
            id: account_id,
            user_id,
            account_number: account_number.clone(),
            balance: initial_balance,
            currency: currency.to_string(),
            account_type,
            is_active: true,
            created_at: Utc::now(),
        };

        // Insert account into database
        sqlx::query(
            r#"
            INSERT INTO accounts (id, user_id, account_number, balance, currency, account_type, is_active, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(&account.id)
        .bind(&account.user_id)
        .bind(&account.account_number)
        .bind(account.balance)
        .bind(&account.currency)
        .bind(account.account_type.to_string())
        .bind(account.is_active)
        .bind(account.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to create account: {}", e)))?;

        Ok(account)
    }

    /// Gets account by account number
    pub async fn get_account_by_number(&self, account_number: &str) -> Result<Option<Account>, PaymentError> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, account_number, balance, currency, account_type, is_active, created_at
            FROM accounts
            WHERE account_number = $1
            "#
        )
        .bind(account_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get account: {}", e)))?;

        if let Some(row) = row {
            let account_type_str: String = row.get("account_type");
            let account_type = match account_type_str.as_str() {
                "CHECKING" => AccountType::Checking,
                "SAVINGS" => AccountType::Savings,
                "CREDIT" => AccountType::Credit,
                "DEBIT" => AccountType::Debit,
                _ => return Err(PaymentError::DatabaseError("Invalid account type".to_string())),
            };

            Ok(Some(Account {
                id: row.get("id"),
                user_id: row.get("user_id"),
                account_number: row.get("account_number"),
                balance: row.get("balance"),
                currency: row.get("currency"),
                account_type,
                is_active: row.get("is_active"),
                created_at: row.get("created_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Gets all accounts for a user
    pub async fn get_user_accounts(&self, user_id: Uuid) -> Result<Vec<Account>, PaymentError> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, account_number, balance, currency, account_type, is_active, created_at
            FROM accounts
            WHERE user_id = $1 AND is_active = true
            ORDER BY created_at DESC
            "#
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get user accounts: {}", e)))?;

        let mut accounts = Vec::new();
        for row in rows {
            let account_type_str: String = row.get("account_type");
            let account_type = match account_type_str.as_str() {
                "CHECKING" => AccountType::Checking,
                "SAVINGS" => AccountType::Savings,
                "CREDIT" => AccountType::Credit,
                "DEBIT" => AccountType::Debit,
                _ => continue, // Skip invalid account types
            };

            accounts.push(Account {
                id: row.get("id"),
                user_id: row.get("user_id"),
                account_number: row.get("account_number"),
                balance: row.get("balance"),
                currency: row.get("currency"),
                account_type,
                is_active: row.get("is_active"),
                created_at: row.get("created_at"),
            });
        }

        Ok(accounts)
    }

    /// Updates account balance
    pub async fn update_account_balance(&self, account_id: Uuid, new_balance: i64) -> Result<(), PaymentError> {
        sqlx::query(
            r#"
            UPDATE accounts
            SET balance = $1
            WHERE id = $2
            "#
        )
        .bind(new_balance)
        .bind(account_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to update account balance: {}", e)))?;

        Ok(())
    }

    /// Checks if user has sufficient funds
    pub async fn check_sufficient_funds(&self, account_number: &str, amount: i64) -> Result<bool, PaymentError> {
        let account = self.get_account_by_number(account_number).await?;
        
        match account {
            Some(acc) => Ok(acc.balance >= amount),
            None => Err(PaymentError::AccountNotFound("Account not found".to_string())),
        }
    }

    /// Transfers money between accounts
    pub async fn transfer_money(
        &self,
        from_account: &str,
        to_account: &str,
        amount: i64,
    ) -> Result<(), PaymentError> {
        // Start a transaction
        let mut tx = self.pool.begin().await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to start transaction: {}", e)))?;

        // Get both accounts
        let from_acc = sqlx::query(
            "SELECT id, balance FROM accounts WHERE account_number = $1 FOR UPDATE"
        )
        .bind(from_account)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get from account: {}", e)))?;

        let to_acc = sqlx::query(
            "SELECT id, balance FROM accounts WHERE account_number = $1 FOR UPDATE"
        )
        .bind(to_account)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to get to account: {}", e)))?;

        let from_account_id: uuid::Uuid = match from_acc {
            Some(acc) => {
                let balance: i64 = acc.get("balance");
                if balance < amount {
                    return Err(PaymentError::InsufficientFunds("Insufficient funds".to_string()));
                }
                acc.get("id")
            }
            None => return Err(PaymentError::AccountNotFound("From account not found".to_string())),
        };

        let to_account_id: uuid::Uuid = match to_acc {
            Some(acc) => acc.get("id"),
            None => return Err(PaymentError::AccountNotFound("To account not found".to_string())),
        };

        // Update balances
        sqlx::query(
            "UPDATE accounts SET balance = balance - $1 WHERE id = $2"
        )
        .bind(amount)
        .bind(from_account_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to update from account: {}", e)))?;

        sqlx::query(
            "UPDATE accounts SET balance = balance + $1 WHERE id = $2"
        )
        .bind(amount)
        .bind(to_account_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| PaymentError::DatabaseError(format!("Failed to update to account: {}", e)))?;

        // Commit transaction
        tx.commit().await
            .map_err(|e| PaymentError::DatabaseError(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// Generates a unique account number
    fn generate_account_number(&self) -> String {
        // In a real system, this would be more sophisticated
        // For now, we'll generate a simple account number
        format!("ACC{:010}", rand::random::<u32>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_account_number() {
        // Mock test - just test the account number generation logic
        let account_number = format!("ACC{:010}", rand::random::<u32>());
        assert!(account_number.starts_with("ACC"));
        assert_eq!(account_number.len(), 13); // "ACC" + 10 digits
    }
}
