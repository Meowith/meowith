use charybdis::macros::charybdis_model;
use charybdis::types::{BigInt, Inet, Text, Timestamp, Uuid};

#[charybdis_model(
    table_name = storage_nodes,
    partition_keys = [id],
    clustering_keys = [],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct StorageNode {
    id: Uuid,
    max_space: BigInt, // bytes
    address: Inet,
    created: Timestamp,
    register_code: Text,
}