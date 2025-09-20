// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "channel_type"))]
    pub struct ChannelType;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ChannelType;

    channel_tbl (id) {
        #[max_length = 255]
        id -> Varchar,
        chan_name -> Nullable<Text>,
        channel_type -> ChannelType,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    user_tbl (id) {
        #[max_length = 255]
        id -> Varchar,
        #[max_length = 50]
        username -> Varchar,
        #[max_length = 100]
        email -> Varchar,
        #[max_length = 255]
        password_hash -> Varchar,
        created_at -> Timestamptz,
    }
}

diesel::allow_tables_to_appear_in_same_query!(channel_tbl, user_tbl,);
