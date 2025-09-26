use chrono::{DateTime, Utc};
use diesel::prelude::*;

use crate::schema::channel_member_tbl;

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = channel_member_tbl)]
pub struct NewChannelMember {
    pub channel_id: String,
    pub user_id: String,
    pub joined_at: DateTime<Utc>,
}
