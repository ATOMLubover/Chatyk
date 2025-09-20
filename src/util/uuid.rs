use uuid::Uuid;

pub async fn generate_id() -> String {
    return Uuid::now_v7().to_string();
}
