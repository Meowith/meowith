use charybdis::macros::charybdis_model;
use charybdis::types::{BigInt, Inet, Text, Timestamp, Uuid};

#[charybdis_model(
    table_name = storage_nodes,
    partition_keys = [cluster_name],
    clustering_keys = [id],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct StorageNode {
    pub cluster_name: Text,
    pub id: Uuid,
    pub max_space: BigInt, // bytes
    pub address: Inet,
    pub created: Timestamp,
    pub register_code: Text,
}
