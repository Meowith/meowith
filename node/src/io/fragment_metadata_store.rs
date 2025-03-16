use bincode::{Decode, Encode};
use commons::error::io_error::MeowithIoResult;
use uuid::Uuid;

#[derive(Encode, Decode, Clone, Copy, Debug)]
pub struct ExtFragmentMeta {
    pub(crate) bucket_id: u128,
    pub(crate) file_id: u128,
}

pub trait ExtFragmentMetaStore: Send + Sync {
    fn insert(&self, chunk_id: Uuid, meta: ExtFragmentMeta) -> MeowithIoResult<()>;

    fn get(&self, chunk_id: &Uuid) -> MeowithIoResult<ExtFragmentMeta>;

    fn remove(&self, chunk_id: &Uuid) -> MeowithIoResult<()>;
}
