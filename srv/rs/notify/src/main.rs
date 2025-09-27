use notify;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    notify::initialize_env()?;

    notify::initialize_logger();

    let config = notify::initialize_config()?;

    let database = notify::initialize_database(&config)?;

    let cache = notify::initialize_cache(&config)?;

    let message_queue = notify::initialize_msgqueue(&config)?;

    notify::serve(config, database, cache, message_queue).await?;

    return Ok(());
}
