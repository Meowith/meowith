use charybdis::macros::{charybdis_model, charybdis_udt_model};
use charybdis::types::{BigInt, Boolean, Counter, Frozen, Set, Text, Timestamp, TinyInt, Uuid};

#[charybdis_udt_model(type_name = file_chunk)]
#[derive(Hash, Eq, PartialEq)]
pub struct FileChunk {
    pub server_id: Uuid,
    pub chunk_id: Uuid,
    pub chunk_size: BigInt,
    pub chunk_order: TinyInt,
}

#[charybdis_model(
    table_name = files,
    partition_keys = [bucket_id],
    clustering_keys = [directory, name],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct File {
    pub bucket_id: Uuid,
    pub directory: Text,
    pub name: Text,
    pub size: BigInt,
    pub chunk_ids: Set<Frozen<FileChunk>>,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

#[charybdis_model(
    table_name = buckets,
    partition_keys = [app_id],
    clustering_keys = [name],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
pub struct Bucket {
    pub app_id: Uuid,
    pub name: Text,
    pub id: Uuid,
    pub encrypted: Boolean,
    pub quota: BigInt,       // in bytes
    pub file_count: Counter, // avoid querying count(*)
    pub created: Timestamp,
    pub last_modified: Timestamp,
}
