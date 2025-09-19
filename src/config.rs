use anyhow::{Error, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub port: u16,

    pub jwt_encoding_key_env: String,
    pub jwt_decoding_key_env: String,
    pub jwt_expiration_hours: i64,

    pub database_url_env: String,
}

impl AppConfig {
    pub fn try_default_load() -> Result<Self, Error> {
        let curr_dir_buf = std::env::current_exe()
            .map_err(|err| anyhow::anyhow!("Failed to get current exe path: {err}."))?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Failed to get parent directory of current exe."))?
            .to_path_buf();

        let config_path_buf = curr_dir_buf.join("app_config.json");

        let config_path = config_path_buf.to_str().ok_or_else(|| {
            anyhow::anyhow!("Failed to convert config path to string: {config_path_buf:?}.")
        })?;

        let config_str = std::fs::read_to_string(config_path)
            .map_err(|err| anyhow::anyhow!("Failed to read config file: {err}."))?;

        let config: AppConfig = serde_json::from_str(&config_str)
            .map_err(|err| anyhow::anyhow!("Failed to parse config file: {err}."))?;

        return Ok(config);
    }
}
