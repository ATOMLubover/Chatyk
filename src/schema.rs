// @generated automatically by Diesel CLI.

diesel::table! {
    channel_member_tbl (channel_id, user_id) {
        #[max_length = 255]
        channel_id -> Varchar,
        #[max_length = 255]
        user_id -> Varchar,
        joined_at -> Timestamptz,
    }
}

diesel::table! {
    channel_tbl (id) {
        #[max_length = 255]
        id -> Varchar,
        chan_name -> Nullable<Text>,
        #[max_length = 63]
        channel_type -> Varchar,
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

diesel::joinable!(channel_member_tbl -> channel_tbl (channel_id));
diesel::joinable!(channel_member_tbl -> user_tbl (user_id));

diesel::allow_tables_to_appear_in_same_query!(channel_member_tbl, channel_tbl, user_tbl,);
