use anyhow::Result;
use redis::{AsyncCommands, Client};
use shared::PaymentError;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
pub struct RedisService {
    client: Client,
}

impl RedisService {
    pub async fn new() -> Result<Self, PaymentError> {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());

        let client = Client::open(redis_url)
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        Ok(Self { client })
    }

    pub async fn health_check(&self) -> Result<(), PaymentError> {
        let mut conn = self.client
            .get_connection_manager()
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        Ok(())
    }

    pub async fn check_idempotency(&self, key: &str) -> Result<(), PaymentError> {
        let mut conn = self.client
            .get_connection_manager()
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        let exists: bool = conn.exists(key).await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        if exists {
            return Err(PaymentError::DuplicateTransaction(key.to_string()));
        }

        Ok(())
    }

    pub async fn set_idempotency_lock(&self, key: &str, transaction_id: &Uuid) -> Result<(), PaymentError> {
        let mut conn = self.client
            .get_connection_manager()
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        // Set lock with 5 minute TTL
        let ttl = Duration::from_secs(300);
        conn.set_ex::<_, _, ()>(key, transaction_id.to_string(), ttl.as_secs()).await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        Ok(())
    }

    pub async fn acquire_lock(&self, lock_key: &str, ttl_seconds: u64) -> Result<bool, PaymentError> {
        let mut conn = self.client
            .get_connection_manager()
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        let lock_value = format!("lock:{}", uuid::Uuid::new_v4());
        
        // Use basic Redis SET command with NX and EX
        let result: String = redis::cmd("SET")
            .arg(lock_key)
            .arg(&lock_value)
            .arg("NX")
            .arg("EX")
            .arg(ttl_seconds)
            .query_async(&mut conn)
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        Ok(result == "OK")
    }

    pub async fn release_lock(&self, lock_key: &str, lock_value: &str) -> Result<(), PaymentError> {
        let mut conn = self.client
            .get_connection_manager()
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        let script = r#"
            if redis.call("GET", KEYS[1]) == ARGV[1] then
                return redis.call("DEL", KEYS[1])
            else
                return 0
            end
        "#;

        redis::cmd("EVAL")
            .arg(script)
            .arg(1)
            .arg(lock_key)
            .arg(lock_value)
            .query_async::<_, i32>(&mut conn)
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        Ok(())
    }

    pub async fn publish_event(&self, channel: &str, event: &str) -> Result<(), PaymentError> {
        let mut conn = self.client
            .get_connection_manager()
            .await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        conn.publish::<_, _, ()>(channel, event).await
            .map_err(|e| PaymentError::RedisError(e.to_string()))?;

        Ok(())
    }
}