use crate::pathlib::join_parent_name;
use charybdis::macros::{charybdis_model, charybdis_udt_model};
use charybdis::types::{BigInt, Boolean, Frozen, Set, Text, Timestamp, TinyInt, Uuid};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use strum::EnumIter;

#[charybdis_udt_model(type_name = filechunk)]
#[derive(Hash, Eq, PartialEq, Clone, Debug, Default)]
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
    local_secondary_indexes = [id],
    static_columns = []
)]
#[derive(Clone, Debug, Default)]
pub struct File {
    pub bucket_id: Uuid,
    pub directory: Uuid, // Uuid::from_u128(0) for root dir
    pub name: Text,
    pub id: Uuid, // Used for reverse lookup
    pub size: BigInt,
    pub chunk_ids: Set<Frozen<FileChunk>>,
    pub created: Timestamp,
    pub last_modified: Timestamp,
}

impl File {
    pub fn full_path(&self, parent: &str) -> String {
        join_parent_name(parent, &self.name)
    }
}

partial_file!(UpdateFileChunks, bucket_id, directory, name, chunk_ids);

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
    pub parent: Text,
    pub name: Text,
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
#[derive(Debug, Clone)]
pub struct Bucket {
    pub app_id: Uuid,
    pub id: Uuid,
    pub name: Text,
    pub encrypted: Boolean,
    pub atomic_upload: Boolean,
    pub quota: BigInt,       // in bytes
    pub file_count: BigInt,  // avoid querying count(*)
    pub space_taken: BigInt, // avoid querying sum(size)
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
            file_count: 0,
            space_taken: 0,
            created: Default::default(),
            last_modified: Default::default(),
        }
    }
}

partial_bucket!(UpdateBucketQuota, app_id, id, quota);

#[charybdis_model(
    table_name = bucket_upload_session,
    partition_keys = [app_id],
    clustering_keys = [bucket, id],
    global_secondary_indexes = [],
    local_secondary_indexes = [],
    static_columns = [],
    table_options = r#"default_time_to_live = 3600;"#
)]
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct BucketUploadSession {
    pub app_id: Uuid,
    pub bucket: Uuid,
    pub id: Uuid,
    pub file_id: Uuid,
    pub path: Text,
    pub size: BigInt,
    pub durable: Boolean,
    pub fragments: Set<Frozen<FileChunk>>,
    pub last_access: Timestamp,
    /// maps to [SessionState]
    pub state: TinyInt,
}

#[derive(Debug, Hash, Eq, PartialEq, EnumIter, IntoPrimitive, TryFromPrimitive, Clone, Copy)]
#[repr(i8)]
/// Represents the current [BucketUploadSession] state.
pub enum SessionState {
    /// The session is being currently written to, no other node may touch it.
    Writing = 1i8,
    /// The session has just been started, or resumed.
    AwaitingData = 2i8,
    /// An error occurred, and the session is awaiting being resumed or auto deletion.
    TimedOut = 3i8,
}

impl Default for BucketUploadSession {
    fn default() -> Self {
        BucketUploadSession {
            app_id: Default::default(),
            bucket: Default::default(),
            file_id: Default::default(),
            id: Default::default(),
            path: "".to_string(),
            size: 0,
            durable: false,
            fragments: Default::default(),
            last_access: Default::default(),
            state: 0,
        }
    }
}
