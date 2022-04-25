table! {
    upload_cells (id) {
        id -> Int4,
        upload_id -> Int4,
        header -> Bool,
        row_num -> Int8,
        column_num -> Int8,
        contents -> Text,
    }
}

table! {
    uploads (id) {
        id -> Int4,
    }
}

joinable!(upload_cells -> uploads (upload_id));

allow_tables_to_appear_in_same_query!(
    upload_cells,
    uploads,
);
