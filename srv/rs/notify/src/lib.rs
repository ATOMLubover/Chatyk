mod cache;
mod handler;
mod msgqueue;
mod service;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{self};
use diesel::PgConnection;
use diesel::r2d2::ConnectionManager;
use r2d2::Pool;
use serde::Deserialize;
use serde_json::{self};
use tokio::net::TcpListener;
use tokio::signal;

use cache::CacheCli;

type DatabasePool = Arc<Pool<ConnectionManager<PgConnection>>>;
type CacheClient = Arc<CacheCli>;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub jwt_deckey_env: String,
    pub database_url_env: String,
    pub cache_url_env: String,
    pub mq_url_env: String,
}

impl AppConfig {
    pub fn try_load_default() -> anyhow::Result<Self> {
        let curr_dir_buf = std::env::current_exe()
            .map_err(|err| anyhow::anyhow!("Failed to get current exe path: {}", err))?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Failed to get parent directory of current exe"))?
            .to_path_buf();

        let config_path_buf = curr_dir_buf.join("notify_config.json");

        let config_path = config_path_buf.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to convert config path to string: {:?}",
                config_path_buf
            )
        })?;

        let config_str = std::fs::read_to_string(config_path)
            .map_err(|err| anyhow::anyhow!("Failed to read config file: {}", err))?;

        let config: AppConfig = serde_json::from_str(&config_str)
            .map_err(|err| anyhow::anyhow!("Failed to parse config file: {}", err))?;

        return Ok(config);
    }
}

pub fn initialize_config() -> anyhow::Result<AppConfig> {
    let config = AppConfig::try_load_default()?;
    return Ok(config);
}

pub fn initialize_env() -> anyhow::Result<()> {
    dotenvy::dotenv().map_err(|err| anyhow::anyhow!("Failed to load .env file: {}", err))?;
    return Ok(());
}

pub fn initialize_logger() {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt;
    use tracing_subscriber::prelude::*;

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .init();

    tracing::debug!("Logger initialized");
}

pub fn initialize_database(config: &AppConfig) -> anyhow::Result<DatabasePool> {
    let database_url = std::env::var(&config.database_url_env)
        .map_err(|err| anyhow::anyhow!("Failed to get database url from env: {err}"))?;

    let manager = ConnectionManager::<PgConnection>::new(database_url);

    let pool = Pool::builder()
        .build(manager)
        .map_err(|err| anyhow::anyhow!(err))?;

    tracing::debug!("Database connection pool created successfully");

    return Ok(Arc::new(pool));
}

pub async fn initialize_cache(config: &AppConfig) -> anyhow::Result<CacheClient> {
    let cli = CacheCli::new(config.cache_url_env.clone()).await?;

    tracing::debug!("Connected to Redis cache successfully");

    return Ok(Arc::new(cli));
}

pub async fn serve(
    app_config: AppConfig,
    database: DatabasePool,
    cache: CacheClient,
) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], app_config.port));

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to bind to address {}: {}", addr, err))?;

    tracing::debug!("Server is now listening on {}", addr);

    let app_router = handler::get_router(&app_config, &database, &cache)?;

    axum::serve(listener, app_router.into_make_service())
        .with_graceful_shutdown(async {
            // wait for the CTRL+C signal to shut down the server
            signal::ctrl_c()
                .await
                .expect("Failed to install CTRL + C signal handler");
            tracing::debug!("CTRL + C Signal received, shutting down gracefully...");
        })
        .await
        .map_err(|err| anyhow::anyhow!("Failed to start server on {}: {}", addr, err))?;

    return Ok(());
}
