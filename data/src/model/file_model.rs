use charybdis::macros::{charybdis_model, charybdis_udt_model};
use charybdis::types::{BigInt, Boolean, Counter, Frozen, Set, Text, Timestamp, TinyInt, Uuid};

#[charybdis_udt_model(type_name = file_chunk)]
#[derive(Hash, Eq, PartialEq, Clone)]
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
    clustering_keys = [id],
    global_secondary_indexes = [],
    local_secondary_indexes = [name],
    static_columns = []
)]
pub struct Bucket {
    pub app_id: Uuid,
    pub id: Uuid,
    pub name: Text,
    pub encrypted: Boolean,
    pub atomic_upload: Boolean,
    pub quota: BigInt,        // in bytes
    pub file_count: Counter,  // avoid querying count(*)
    pub space_taken: Counter, // avoid querying sum(size)
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

impl Default for Bucket {
    fn default() -> Self {
        Bucket {
            app_id: Default::default(),
            id: Default::default(),
            name: "".to_string(),
            encrypted: false,
            atomic_upload: false,
            quota: 0,
            file_count: Counter::default(),
            space_taken: Counter::default(),
            created: Default::default(),
            last_modified: Default::default(),
        }
    }
}
