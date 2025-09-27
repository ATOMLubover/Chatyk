pub use redis::*;

use anyhow::{Error, Result};
use redis::aio::MultiplexedConnection;

pub type MqError = RedisError;

pub type MqResult<T> = Result<T, MqError>;

pub type MqAsyncConn = MultiplexedConnection;
pub type MqSyncConn = Connection;

/// MessageQueueCli is now just a simple wrapper around redis::Client
/// it does not provide any additional functionality
#[derive(Debug)]
pub struct MessageQueueCli {
    redis_cli: Client,
}

impl MessageQueueCli {
    pub fn new(env_var: String) -> Result<Self, Error> {
        let redis_url = std::env::var(env_var)
            .map_err(|err| anyhow::anyhow!("Failed to get REDIS_URL from env: {err}."))?;

        let redis_cli = Client::open(redis_url)
            .map_err(|err| anyhow::anyhow!("Failed to create Redis client: {}.", err))?;

        // test the connection with a ping
        let conn = &mut redis_cli
            .get_connection()
            .map_err(|err| anyhow::anyhow!("Failed to connect to Redis: {}.", err))?;

        match redis::cmd("PING").query::<String>(conn) {
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

        Ok(MessageQueueCli { redis_cli })
    }

    pub async fn get_async_conn(&self) -> MqResult<MqAsyncConn> {
        let conn = self.redis_cli.get_multiplexed_async_connection().await?;
        return Ok(conn);
    }

    pub fn get_sync_conn(&self) -> MqResult<MqSyncConn> {
        let conn = self.redis_cli.get_connection()?;
        return Ok(conn);
    }
}
