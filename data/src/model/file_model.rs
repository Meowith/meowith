use charybdis::macros::{charybdis_model, charybdis_udt_model};
use charybdis::types::{BigInt, Boolean, Counter, Frozen, Set, Text, Timestamp, TinyInt, Uuid};

#[charybdis_udt_model(type_name = file_chunk)]
#[derive(Hash, Eq, PartialEq)]
pub struct FileChunk {
    server_id: Uuid,
    chunk_id: Uuid,
    chunk_size: BigInt,
    chunk_order: TinyInt,
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
    bucket_id: Uuid,
    directory: Text,
    name: Text,
    size: BigInt,
    chunk_ids: Set<Frozen<FileChunk>>,
    created: Timestamp,
    last_modified: Timestamp,
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
    app_id: Uuid,
    name: Text,
    id: Uuid,
    encrypted: Boolean,
    quota: BigInt,       // in bytes
    file_count: Counter, // avoid querying count(*)
    created: Timestamp,
    last_modified: Timestamp,
}
