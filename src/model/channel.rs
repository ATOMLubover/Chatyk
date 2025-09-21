use chrono::{DateTime, Utc};
use diesel::prelude::*;

use crate::schema::channel_tbl;

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = channel_tbl)]
pub struct NewChannel {
    pub id: String,
    pub chan_name: String,
    pub channel_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Selectable, Queryable)]
#[diesel(table_name = channel_tbl)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChannelInfo {
    pub id: String,
    pub chan_name: Option<String>,
    pub channel_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
