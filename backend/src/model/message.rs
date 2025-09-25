use crate::schema::message_tbl;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = message_tbl)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewMessage {
    pub id: String,
    pub channel_id: String,
    pub sender_id: String,
    /// content is a rich text xml string
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Selectable, Queryable, Serialize)]
#[diesel(table_name = message_tbl)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MessageInfo {
    pub id: String,
    pub channel_id: String,
    pub sender_id: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
