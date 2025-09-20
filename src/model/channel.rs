use diesel::expression::AsExpression;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::channel_tbl;
use crate::schema::sql_types::ChannelType;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, AsExpression)]
#[diesel(sql_type = ChannelType)]
pub enum ChanType {
    Private,
    Public,
}

#[derive(Debug, Clone, Selectable, Queryable)]
#[diesel(table_name = channel_tbl)]
pub struct NewChannel {
    pub id: String,
    pub chan_name: String,
    pub channel_type: ChanType,
    pub created_at: i64,
}
