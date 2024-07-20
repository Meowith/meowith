use charybdis::macros::{charybdis_model, charybdis_view_model};
use charybdis::types::{BigInt, Set, Text, Timestamp, Uuid};

use crate::model::permission_model::UserPermission;

#[charybdis_model(
    table_name = apps,
    partition_keys = [id],
    clustering_keys = [],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct App {
    pub id: Uuid,
    pub name: Text,
    pub owner_id: Uuid,
    pub quota: BigInt,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

#[charybdis_view_model(
    table_name = apps_by_owner,
    base_table = apps,
    partition_keys = [owner_id],
    clustering_keys = [id]
)]
pub struct AppByOwner {
    pub owner_id: Uuid,
    pub id: Uuid,
    pub name: Text,
    pub quota: BigInt,
    pub created: Timestamp,
    pub last_modified: Timestamp,
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
    pub scopes: Set<(Text, UserPermission)>,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

#[charybdis_model(
    table_name = app_members,
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

#[charybdis_model(
    table_name = app_tokens,
    partition_keys = [app_id],
    clustering_keys = [issuer_id, name],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct AppToken {
    pub app_id: Uuid,
    pub issuer_id: Uuid,
    pub name: Text,
    pub nonce: Uuid,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}
