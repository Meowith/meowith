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
    pub id: Uuid,
    pub name: Text,
    pub global_role: Int,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

// We are using a view because we will not know the partition key beforehand.
// This prevents requests to all nodes in a cluster
#[charybdis_view_model(
    table_name = users_by_name,
    base_table = users,
    partition_keys = [name],
    clustering_keys = [id]
)]
pub struct UsersByName {
    pub name: Text,
    pub id: Uuid,
    pub global_role: Int,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

impl From<UsersByName> for User {
    fn from(value: UsersByName) -> Self {
        User {
            id: value.id,
            name: value.name,
            global_role: value.global_role,
            created: value.created,
            last_modified: value.last_modified,
        }
    }
}