use std::collections::HashMap;
use std::sync::Arc;

use tokio::fs::File;
use tokio::io::{BufReader, BufWriter};
use uuid::Uuid;

use crate::io::error::{MeowithIoError, MeowithIoResult};
use crate::locking::file_lock_table::FileLockTable;

pub type LockTable = FileLockTable<Uuid>;

pub type FragmentReadStream = BufReader<File>;
pub type FragmentWriteStream = BufWriter<File>;

#[derive(Clone)]
pub struct FragmentLedger {
    _internal: Arc<InternalLedger>,
}

#[allow(unused)]
impl FragmentLedger {
    pub fn new(root_path: String, file_lock_table: LockTable) -> Self {
        FragmentLedger {
            _internal: Arc::new(InternalLedger {
                root_path,
                file_lock_table,
                chunk_set: Default::default(),
            }),
        }
    }

    pub fn initialize(&self) {
        todo!()
    }

    pub fn lock_table(&self) -> LockTable {
        self._internal.file_lock_table.clone()
    }

    pub fn fragment_exists(&self, id: &Uuid) -> bool {
        self._internal.chunk_set.contains_key(id)
    }

    fn get_path(&self, id: &Uuid) -> String {
        self._internal.root_path.clone() + &*id.to_string()
    }

    pub async fn fragment_read_stream(&self, id: &Uuid) -> MeowithIoResult<FragmentReadStream> {
        let file = File::open(self.get_path(id)).await.map_err(MeowithIoError::from)?;
        Ok(BufReader::new(file))
    }

    pub async fn fragment_write_stream(&self, id: &Uuid) -> MeowithIoResult<FragmentWriteStream> {
        let file = File::open(self.get_path(id)).await.map_err(MeowithIoError::from)?;
        Ok(BufWriter::new(file))
    }

    pub fn get_available_space() -> u64 {
        todo!()
    }
}

#[allow(unused)]
struct FragmentMeta {
    size: u64,
}

struct InternalLedger {
    root_path: String,
    file_lock_table: LockTable,
    chunk_set: HashMap<Uuid, FragmentMeta>,
}
