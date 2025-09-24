mod cache;
mod config;
mod dto;
mod handler;
mod model;
mod schema;
mod service;
mod util;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Error, Result};
use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use tokio::net::TcpListener;
use tokio::signal;

use crate::cache::Cache;
use crate::config::AppConfig;

type DbPool = Arc<Pool<ConnectionManager<PgConnection>>>;
type CacheCli = Arc<Cache>;

pub fn initialize_logger() -> Result<(), Error> {
    // load .env file, in order to read RUST_LOG env variable
    dotenvy::dotenv().map_err(|err| anyhow::anyhow!("Failed to load .env file: {err}."))?;

    // create a default subscriber that logs to stdout
    // its log level is set by the RUST_LOG env variable
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .try_init()
        .map_err(|err| anyhow::anyhow!(err))?;

    return Ok(());
}

pub fn initialize_config() -> Result<AppConfig, Error> {
    let config = AppConfig::try_default_load()?;
    return Ok(config);
}

pub fn initialize_database(config: &AppConfig) -> Result<DbPool, Error> {
    // load .env file
    dotenvy::dotenv().map_err(|err| anyhow::anyhow!("Failed to load .env file: {err}."))?;

    // then use std::env to read the env variable safely
    let database_url = std::env::var(&config.database_url_env)
        .map_err(|err| anyhow::anyhow!("Failed to get database url from env: {err}."))?;

    let manager = ConnectionManager::<PgConnection>::new(database_url);

    let pool = Pool::builder()
        .build(manager)
        .map_err(|err| anyhow::anyhow!(err))?;

    return Ok(Arc::new(pool));
}

pub async fn initialize_cache(config: &AppConfig) -> Result<CacheCli, Error> {
    let cache = Cache::new(config.redis_url_env.clone()).await?;

    return Ok(Arc::new(cache));
}

pub async fn serve(config: AppConfig, dbpool: DbPool, cache_cli: CacheCli) -> Result<(), Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to bind to address {addr}: {err}"))?;

    tracing::debug!("Server is now listening on {}", addr);

    let app_router = handler::get_router(config, dbpool, cache_cli)?;

    axum::serve(listener, app_router.into_make_service())
        .with_graceful_shutdown(async {
            tracing::debug!("Press CTRL + C to shut down the server gracefully...");

            // wait for the CTRL+C signal to shut down the server
            signal::ctrl_c()
                .await
                .expect("Failed to install CTRL + C signal handler");

            tracing::debug!("CTRL + C Signal received, shutting down gracefully...");
        })
        .await
        .map_err(|err| anyhow::anyhow!("Failed to start server on {addr}: {err}"))?;

    return Ok(());
}
