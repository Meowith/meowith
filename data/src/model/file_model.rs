use charybdis::macros::{charybdis_model, charybdis_udt_model};
use charybdis::types::{BigInt, Boolean, Counter, Frozen, Set, Text, Timestamp, TinyInt, Uuid};
use crate::pathlib::join_parent_name;
// partial_directory!(
//     UpdateDirectory,
//     id,
//     microservice_type,
//     used_space,
//     max_space
// );

#[charybdis_udt_model(type_name = file_chunk)]
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
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
#[derive(Clone)]
pub struct File {
    pub bucket_id: Uuid,
    pub directory: Uuid, // Uuid::from_u128(0) for root dir
    pub name: Text,
    pub size: BigInt,
    pub chunk_ids: Set<Frozen<FileChunk>>,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

#[charybdis_model(
    table_name = directories,
    partition_keys = [bucket_id],
    clustering_keys = [parent, name],
    global_secondary_indexes = [],
    local_secondary_indexes = [id],
    static_columns = []
)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Directory {
    pub bucket_id: Uuid,
    pub parent: String,
    pub name: String,
    pub id: Uuid,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

impl Directory {
    pub fn full_path(&self) -> String {
        join_parent_name(&self.parent, &self.name)
    }
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

#[charybdis_model(
    table_name = bucket_upload_session,
    partition_keys = [app_id],
    clustering_keys = [bucket, id],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = []
)]
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct BucketUploadSession {
    pub app_id: Uuid,
    pub bucket: Uuid,
    pub id: Uuid,
    pub path: Text,
    pub size: BigInt,
    pub durable: Boolean,
    pub fragments: Set<Frozen<FileChunk>>,
    pub last_access: Timestamp,
}

partial_bucket_upload_session!(UpdateBucketUploadSession, app_id, bucket, id, last_access);

impl Default for BucketUploadSession {
    fn default() -> Self {
        BucketUploadSession {
            app_id: Default::default(),
            bucket: Default::default(),
            id: Default::default(),
            path: "".to_string(),
            size: 0,
            durable: false,
            fragments: Default::default(),
            last_access: Default::default(),
        }
    }
}
