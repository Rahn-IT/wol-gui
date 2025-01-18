// @generated automatically by Diesel CLI.

diesel::table! {
    wol (id) {
        id -> Nullable<Integer>,
        name -> Text,
        mac -> Text,
        ip -> Nullable<Text>,
    }
}
