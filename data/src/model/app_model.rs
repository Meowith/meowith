use charybdis::macros::charybdis_model;
use charybdis::types::{BigInt, Set, Text, Timestamp, Uuid};

use crate::model::permission_model::UserPermission;

#[charybdis_model(
    table_name = apps,
    partition_keys = [owner_id],
    clustering_keys = [id],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct App {
    owner_id: Uuid,
    id: Uuid,
    name: Text,
    quota: BigInt,
    created: Timestamp,
    last_modified: Timestamp,
    members: Set<Uuid>
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
    app_id: Uuid,
    name: Text, // non-re-nameable
    scopes: Set<Text>,
    permissions: Set<UserPermission>, // tinyint aka out mapper permission enum literal
    created: Timestamp,
    last_modified: Timestamp,
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
    app_id: Uuid,
    member_id: Uuid,
    member_roles: Set<Text>
}