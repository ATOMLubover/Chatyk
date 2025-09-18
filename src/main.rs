use anyhow::{Error, Result};

use chatyk;

#[tokio::main]
async fn main() -> Result<(), Error> {
    chatyk::initialize_logger()?;

    let config = chatyk::initialize_config()?;

    let db_pool = chatyk::initialize_database(&config)?;

    chatyk::serve(config, db_pool).await?;

    return Ok(());
}
