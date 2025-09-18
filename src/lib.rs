mod config;
mod dto;
mod error;
mod handler;
mod shared;

use std::net::SocketAddr;

use anyhow::{Error, Result};
use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use tokio::net::TcpListener;
use tokio::signal;

use crate::config::AppConfig;

type DbPool = diesel::r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn initialize_logger() -> Result<(), Error> {
    // create a default subscriber that logs to stdout
    // its log level is set by the RUST_LOG env variable
    tracing_subscriber::fmt()
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

    tracing::trace!("Database URL: {database_url}");

    let manager = ConnectionManager::<PgConnection>::new(database_url);

    let pool = Pool::builder()
        .build(manager)
        .map_err(|err| anyhow::anyhow!(err))?;

    return Ok(pool);
}

pub async fn serve(config: AppConfig, db_pool: DbPool) -> Result<(), Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to bind to address {addr}: {err}"))?;

    tracing::info!("Server is now listening on {}", addr);

    let app_router = handler::get_router();

    axum::serve(listener, app_router.into_make_service())
        .with_graceful_shutdown(async {
            // wait for the CTRL+C signal to shut down the server
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        })
        .await
        .map_err(|err| anyhow::anyhow!("Failed to start server on {addr}: {err}"))?;

    return Ok(());
}
