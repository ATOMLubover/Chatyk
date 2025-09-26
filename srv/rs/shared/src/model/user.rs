use chrono::{DateTime, Utc};
use diesel::prelude::*;

use crate::schema::user_tbl;

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = user_tbl)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewUser {
    pub id: String,
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, Selectable, Queryable)]
#[diesel(table_name = user_tbl)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub created_at: DateTime<Utc>,
}
