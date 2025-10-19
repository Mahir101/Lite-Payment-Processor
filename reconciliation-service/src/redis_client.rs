use anyhow::Result;
use redis::{AsyncCommands, Client};

#[derive(Clone)]
pub struct RedisService {
    client: Client,
}

impl RedisService {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string());

        let client = Client::open(redis_url)?;
        Ok(Self { client })
    }

    pub async fn health_check(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.client
            .get_connection_manager()
            .await?;

        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await?;

        Ok(())
    }

    pub async fn subscribe_to_events(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Simplified implementation - just return Ok for now
        // In a real implementation, this would handle Redis pub/sub
        Ok(())
    }

    pub async fn publish_event(&self, channel: &str, event: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.client
            .get_connection_manager()
            .await?;

        conn.publish::<_, _, ()>(channel, event).await?;
        Ok(())
    }
}