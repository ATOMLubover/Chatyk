use anyhow::{Error, Result};
use redis::{Client, Connection, RedisError, aio::MultiplexedConnection};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Redis error: {0}")]
    RedisError(#[from] RedisError),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Unknown error: {0}")]
    UnknownError(String),
}

pub type CacheResult<T> = Result<T, CacheError>;

pub type CacheAsyncConn = MultiplexedConnection;
pub type CacheSyncConn = Connection;

#[derive(Debug)]
pub struct Cache {
    redis_cli: Client,
}

impl Cache {
    pub async fn new(host: String, port: String) -> Result<Self, Error> {
        let redis_cli = Client::open(format!("redis://{}:{}", host, port))
            .map_err(|err| anyhow::anyhow!("Failed to create Redis client: {}.", err))?;

        // test the connection with a ping
        let conn = &mut redis_cli
            .get_multiplexed_async_connection()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to connect to Redis: {}.", err))?;

        match redis::cmd("PING").query_async::<String>(conn).await {
            Ok(pong) if pong != "PONG" => {
                return Err(anyhow::anyhow!(
                    "Failed to ping Redis, unexpected response: {}.",
                    pong
                ));
            }
            Err(err) => {
                return Err(anyhow::anyhow!("Failed to ping Redis: {}.", err));
            }
            _ => (),
        };

        Ok(Cache { redis_cli })
    }

    pub async fn get_async_conn(&self) -> CacheResult<CacheAsyncConn> {
        let conn = self.redis_cli.get_multiplexed_async_connection().await?;

        return Ok(conn);
    }

    pub fn get_sync_conn(&self) -> CacheResult<Connection> {
        let conn = self.redis_cli.get_connection()?;

        return Ok(conn);
    }
}
