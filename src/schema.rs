// @generated automatically by Diesel CLI.

diesel::table! {
    email_verification_tokens (id) {
        id -> Nullable<Integer>,
        token -> Text,
        expires_at -> Timestamp,
        created_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Nullable<Integer>,
        email -> Text,
        login_token -> Text,
        login_expires_at -> Timestamp,
        access_token -> Nullable<Text>,
        access_expires_at -> Nullable<Timestamp>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    email_verification_tokens,
    users,
);
