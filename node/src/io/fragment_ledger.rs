use log::{error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::{File, OpenOptions};
use tokio::io::{BufReader, BufWriter};
use tokio::sync::RwLock;
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

    pub async fn initialize(&self) -> MeowithIoResult<()> {
        let chunk_dir = Path::new(&self._internal.root_path);
        if !chunk_dir.exists() {
            info!("Creating the data directory {}", chunk_dir.display());
            fs::create_dir_all(chunk_dir).map_err(MeowithIoError::from)?;
        }
        if !chunk_dir.is_dir() {
            error!(
                "The data directory {} is not a directory",
                chunk_dir.display()
            );
            return Err(MeowithIoError::InvalidDataDir);
        }

        info!("Scanning fragments...");
        let dir_scan = fs::read_dir(chunk_dir).map_err(MeowithIoError::from)?;
        let mut chunk_map = self._internal.chunk_set.write().await;
        let mut last_notify = Instant::now();

        for entry in dir_scan {
            let entry = entry.map_err(MeowithIoError::from)?;
            match Uuid::from_str(entry.file_name().to_str().unwrap_or("invalid_unicode")) {
                Ok(id) => {
                    if let Ok(metadata) = entry.metadata() {
                        chunk_map.insert(
                            id,
                            FragmentMeta {
                                disk_size: metadata.len(),
                            },
                        );
                        if last_notify.elapsed() > Duration::from_secs(5) {
                            info!("Scanned {} entries so far", chunk_map.len());
                        }
                    } else {
                        warn!("Couldn't get metadata for {:?}", entry.file_name());
                    }
                }
                Err(_) => {
                    warn!("Foreign file in data dir {:?}", entry.file_name())
                }
            }
        }

        info!("Found {} fragments.", chunk_map.len());

        Ok(())
    }

    pub fn lock_table(&self) -> LockTable {
        self._internal.file_lock_table.clone()
    }

    pub async fn fragment_exists(&self, id: &Uuid) -> bool {
        self._internal.chunk_set.read().await.contains_key(id)
    }

    fn get_path(&self, id: &Uuid) -> String {
        self._internal.root_path.clone() + &*id.to_string()
    }

    pub async fn fragment_read_stream(&self, id: &Uuid) -> MeowithIoResult<FragmentReadStream> {
        let file = File::open(self.get_path(id))
            .await
            .map_err(MeowithIoError::from)?;
        Ok(BufReader::new(file))
    }

    pub async fn fragment_write_stream(&self, id: &Uuid) -> MeowithIoResult<FragmentWriteStream> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(self.get_path(id))
            .await
            .map_err(MeowithIoError::from)?;
        Ok(BufWriter::new(file))
    }

    pub async fn try_reserve(size: u64) -> MeowithIoResult<Uuid> {
        todo!()
    }

    pub fn get_available_space() -> u64 {
        todo!()
    }
}

#[allow(unused)]
struct FragmentMeta {
    disk_size: u64,
}

struct InternalLedger {
    root_path: String,
    file_lock_table: LockTable,
    chunk_set: RwLock<HashMap<Uuid, FragmentMeta>>,
}
