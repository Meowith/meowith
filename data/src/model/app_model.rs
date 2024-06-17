use charybdis::macros::charybdis_model;
use charybdis::types::{BigInt, Set, Text, Timestamp, Uuid};

use crate::model::permission_model::UserPermission;

// We are using a local index as we will always know the partition key.
// Because of that, we won't be making requests to all nodes in the cluster
#[charybdis_model(
    table_name = apps,
    partition_keys = [owner_id],
    clustering_keys = [id],
    global_secondary_indexes = [],
    local_secondary_indexes = [name],
    static_columns = []
)]
pub struct App {
    pub owner_id: Uuid,
    pub id: Uuid,
    pub name: Text,
    pub quota: BigInt,
    pub created: Timestamp,
    pub last_modified: Timestamp,
    pub members: Set<Uuid>,
}

#[charybdis_model(
    table_name = user_roles,
    partition_keys = [app_id],
    clustering_keys = [name],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct UserRole {
    pub app_id: Uuid,
    pub name: Text, // non-re-nameable
    pub scopes: Set<Text>,
    pub permissions: Set<UserPermission>, // tinyint aka out mapper permission enum literal
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

#[charybdis_model(
    table_name = user_roles,
    partition_keys = [app_id],
    clustering_keys = [member_id],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct AppMember {
    pub app_id: Uuid,
    pub member_id: Uuid,
    pub member_roles: Set<Text>,
}
