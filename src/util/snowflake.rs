use snowflake::ProcessUniqueId;
use tokio::sync::Mutex;

type Generator = fn() -> String;

static SNOWFLAKE_ID: Mutex<Generator> = Mutex::const_new(|| ProcessUniqueId::new().to_string());

pub async fn generate_id() -> String {
    (SNOWFLAKE_ID.lock().await)()
}
