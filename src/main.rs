use anyhow::{Error, Result};

use chatyk;

#[tokio::main]
async fn main() -> Result<(), Error> {
    chatyk::initialize_logger()?;

    let config = chatyk::initialize_config()?;

    let db_pool = chatyk::initialize_database(&config)?;

    let cache = chatyk::initialize_cache(&config).await?;

    chatyk::serve(config, db_pool, cache).await?;

    return Ok(());
}
