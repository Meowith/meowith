use charybdis::macros::{charybdis_model, charybdis_view_model};
use charybdis::types::{Int, Text, Timestamp, Uuid};

#[charybdis_model(
    table_name = users,
    partition_keys = [id],
    clustering_keys = [name],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct User {
    id: Uuid,
    name: Text,
    global_role: Int,
    created: Timestamp,
    last_modified: Timestamp,
}

#[charybdis_view_model(
    table_name = users_by_name,
    base_table = users,
    partition_keys = [name],
    clustering_keys = [id]
)]
pub struct UsersByName {
    name: Text,
    id: Uuid,
    global_role: Int,
    created: Timestamp,
    last_modified: Timestamp,
}
