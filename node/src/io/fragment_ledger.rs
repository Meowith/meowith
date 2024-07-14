use filesize::PathExt;
use log::{error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::fs::{File, OpenOptions};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time;
use uuid::Uuid;

use crate::io::error::{MeowithIoError, MeowithIoResult};
use crate::io::get_space;
use crate::locking::file_lock_table::FileLockTable;

pub type LockTable = FileLockTable<Uuid>;

pub type FragmentReadStream = File;
pub type FragmentWriteStream = File;

#[derive(Clone)]
pub struct FragmentLedger {
    _internal: Arc<InternalLedger>,
}

const ORDERING_MAX_LOAD: Ordering = Ordering::Relaxed;
const ORDERING_DISK_LOAD: Ordering = Ordering::Relaxed;
const ORDERING_DISK_STORE: Ordering = Ordering::SeqCst;

const AVAILABLE_BUFFER: u64 = 65535;

// TODO: verify the fragments should exist with database

#[allow(unused)]
impl FragmentLedger {
    pub fn new(root_path: String, max_space: u64, file_lock_table: LockTable) -> Self {
        let internal = InternalLedger {
            root_path: PathBuf::from(root_path),
            file_lock_table,
            chunk_set: Default::default(),
            max_physical_size: AtomicU64::new(max_space),
            disk_physical_size: Default::default(),
            disk_content_size: Default::default(),
            reservation_map: Default::default(),
            housekeeper_handle: Mutex::new(None),
            disk_reserved_size: Default::default(),
        };

        let internal_arc = Arc::new(internal);
        let housekeeper_arc = internal_arc.clone();

        let binding = internal_arc.clone();
        let mut guard = binding.housekeeper_handle.lock().unwrap();
        *guard = Some(tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5 * 60));

            loop {
                interval.tick().await;
                let _ = housekeeper_arc.validate_max_space().await;
            }
        }));

        FragmentLedger {
            _internal: internal_arc,
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

        self._internal.validate_max_space().await?;
        self.scan_fragments().await?;

        Ok(())
    }

    async fn scan_fragments(&self) -> MeowithIoResult<()> {
        info!("Scanning fragments...");
        let chunk_dir = Path::new(&self._internal.root_path);
        let dir_scan = fs::read_dir(chunk_dir).map_err(MeowithIoError::from)?;
        let mut chunk_map = self._internal.chunk_set.write().await;
        let mut last_notify = Instant::now();

        for entry in dir_scan {
            let entry = entry.map_err(MeowithIoError::from)?;
            let entry_path = entry.path();
            let path = Path::new(&entry_path);
            match Uuid::from_str(entry.file_name().to_str().unwrap_or("invalid_unicode")) {
                Ok(id) => {
                    if let Ok(metadata) = entry.metadata() {
                        chunk_map.insert(
                            id,
                            FragmentMeta {
                                disk_content_size: metadata.len(),
                                disk_physical_size: path
                                    .size_on_disk_fast(&metadata)
                                    .unwrap_or(metadata.len()),
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

    pub async fn fragment_meta(&self, id: &Uuid) -> Option<FragmentMeta> {
        self._internal.chunk_set.read().await.get(id).cloned()
    }

    fn get_path(&self, id: &Uuid) -> PathBuf {
        let mut path = self._internal.root_path.clone();
        path.push(id.to_string());
        path
    }

    pub async fn fragment_read_stream(&self, id: &Uuid) -> MeowithIoResult<FragmentReadStream> {
        let file = File::open(self.get_path(id))
            .await
            .map_err(MeowithIoError::from)?;
        Ok(file)
    }

    pub async fn fragment_write_stream(&self, id: &Uuid) -> MeowithIoResult<FragmentWriteStream> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(self.get_path(id))
            .await
            .map_err(MeowithIoError::from)?;
        Ok(file)
    }

    pub async fn try_reserve(&self, size: u64) -> MeowithIoResult<Uuid> {
        let mut reservations = self._internal.reservation_map.write().await;

        let available = self.get_available_space();
        if available - AVAILABLE_BUFFER < size {
            return Err(MeowithIoError::InsufficientDiskSpace);
        }

        let reservation = Reservation { file_space: size };

        let id = Uuid::new_v4();

        reservations.insert(id, reservation);

        self._internal
            .disk_reserved_size
            .fetch_add(size, ORDERING_DISK_STORE);

        Ok(id)
    }

    pub async fn release_reservation(&self, id: &Uuid, size: u64) -> MeowithIoResult<()> {
        let mut reservations = self._internal.reservation_map.write().await;
        let path = &self.get_path(id);
        let physical_size = Path::new(path)
            .size_on_disk()
            .map_err(|_| MeowithIoError::Internal(None))?;

        self._internal
            .disk_physical_size
            .fetch_add(physical_size, ORDERING_DISK_STORE);
        self._internal
            .disk_content_size
            .fetch_add(size, ORDERING_DISK_STORE);

        reservations.remove(id);

        self._internal
            .disk_reserved_size
            .fetch_sub(size, ORDERING_DISK_STORE);

        drop(reservations);

        let mut chunks = self._internal.chunk_set.write().await;
        chunks.insert(
            *id,
            FragmentMeta {
                disk_content_size: size,
                disk_physical_size: physical_size,
            },
        );

        Ok(())
    }

    pub fn get_available_space(&self) -> u64 {
        let used = self._internal.disk_physical_size.load(ORDERING_DISK_LOAD)
            + self._internal.disk_reserved_size.load(ORDERING_DISK_LOAD);
        let current = self._internal.max_physical_size.load(ORDERING_MAX_LOAD);

        if used > current {
            0
        } else {
            current - used
        }
    }
}

#[allow(unused)]
struct Reservation {
    file_space: u64,
}

#[allow(unused)]
#[derive(Clone, Debug)]
pub struct FragmentMeta {
    pub disk_content_size: u64,
    pub disk_physical_size: u64,
}

#[allow(unused)]
struct InternalLedger {
    root_path: PathBuf,
    file_lock_table: LockTable,
    chunk_set: RwLock<HashMap<Uuid, FragmentMeta>>,
    reservation_map: RwLock<HashMap<Uuid, Reservation>>,

    housekeeper_handle: Mutex<Option<JoinHandle<()>>>,

    max_physical_size: AtomicU64,
    disk_physical_size: AtomicU64,
    disk_content_size: AtomicU64,

    disk_reserved_size: AtomicU64,
}

impl InternalLedger {
    async fn validate_max_space(&self) -> MeowithIoResult<()> {
        let usage = get_space(&self.root_path).await?;
        let physical_used = self.disk_physical_size.load(ORDERING_DISK_LOAD);
        let max = self.max_physical_size.load(ORDERING_MAX_LOAD);

        let app_free = max - physical_used;
        let disk_free = usage.free;

        if disk_free < app_free {
            warn!("Disk free space is not big enough to contain the app limit, file operations can fail");
        }
        Ok(())
    }
}

impl Drop for InternalLedger {
    fn drop(&mut self) {
        match &self.housekeeper_handle.get_mut().unwrap() {
            None => {}
            Some(handle) => handle.abort(),
        }
    }
}
