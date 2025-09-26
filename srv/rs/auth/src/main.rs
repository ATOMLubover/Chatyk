use auth;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    auth::initialize_env()?;

    auth::initialize_logger();

    let config = auth::initialize_config()?;

    let database = auth::initialize_database(&config)?;

    auth::serve(config, database).await?;

    return Ok(());
}
