use filesize::PathExt;
use futures_util::{StreamExt, TryStreamExt};
use log::{error, info, trace, warn};
use scylla::client::caching_session::CachingSession;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::{File, OpenOptions};
use tokio::io::{BufReader, BufStream, BufWriter};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time;
use uuid::Uuid;

use protocol::mdsftp::handler::{AbstractFileStream, AbstractReadStream, AbstractWriteStream};

use crate::io::fragment_metadata_store::{ExtFragmentMeta, ExtFragmentMetaStore};
use crate::io::get_space;
use crate::locking::file_lock_table::FileLockTable;
use crate::public::service::durable_transfer_session_manager::DURABLE_UPLOAD_SESSION_VALIDITY_TIME_SECS;
use commons::error::io_error::{MeowithIoError, MeowithIoResult};
use data::access::file_access::{get_all_files, maybe_get_file_by_id};
use data::dto::controller::UpdateStorageNodeProperties;

pub type LockTable = FileLockTable<Uuid>;

pub type FragmentReadOmniStream = AbstractFileStream;
pub type FragmentReadStream = AbstractReadStream;
pub type FragmentWriteStream = AbstractWriteStream;

#[derive(Clone)]
pub struct FragmentLedger {
    _internal: Arc<InternalLedger>,
}

const ORDERING_MAX_LOAD: Ordering = Ordering::Relaxed;
const ORDERING_DISK_LOAD: Ordering = Ordering::Relaxed;
const ORDERING_DISK_STORE: Ordering = Ordering::SeqCst;

const HOUSEKEEPER_TASK_INTERVAL: usize = 5 * 60;

#[allow(unused)]
const AVAILABLE_BUFFER: u64 = 65535;

impl FragmentLedger {
    pub fn new(
        root_path: String,
        max_space: u64,
        file_lock_table: LockTable,
        ext_metadata_store: Box<dyn ExtFragmentMetaStore>,
    ) -> Self {
        let internal = InternalLedger {
            root_path: PathBuf::from(root_path),
            file_lock_table,
            chunk_set: Default::default(),
            max_physical_size: AtomicU64::new(max_space),
            disk_physical_size: Default::default(),
            disk_content_size: Default::default(),
            reservation_map: Default::default(),
            uncommited_map: Default::default(),
            ext_metadata_store: RwLock::new(Some(ext_metadata_store)),
            housekeeper_handle: std::sync::Mutex::new(None),
            disk_reserved_size: Default::default(),
            paused: AtomicBool::new(false),
        };

        let internal_arc = Arc::new(internal);
        let housekeeper_arc = internal_arc.clone();

        let binding = internal_arc.clone();
        let mut guard = binding.housekeeper_handle.lock().unwrap();
        *guard = Some(tokio::spawn(async move {
            let mut interval =
                time::interval(Duration::from_secs(HOUSEKEEPER_TASK_INTERVAL as u64));

            loop {
                interval.tick().await;
                let _ = housekeeper_arc.validate_max_space().await;
                let _ = housekeeper_arc.clean_broken_chunks().await;
                let _ = housekeeper_arc.clean_uncommitted().await;
            }
        }));

        FragmentLedger {
            _internal: internal_arc,
        }
    }

    /// Pause accepting incoming reservation requests
    /// Does not interrupt ongoing transfers
    #[inline(always)]
    pub fn pause(&self) {
        self._internal.paused.store(true, Ordering::Release);
    }

    /// Resume accepting reservation requests
    #[inline(always)]
    pub fn resume(&self) {
        self._internal.paused.store(false, Ordering::Release);
    }

    /// Pause reservations and shutdown internal components.
    pub async fn shutdown(&self) {
        self._internal.paused.store(true, Ordering::Release);
        if let Some(h) = self._internal.housekeeper_handle.lock().unwrap().take() {
            h.abort();
        }
        // Drop the store, release the lock on the on-disk db
        self._internal.ext_metadata_store.write().await.take();
    }

    pub async fn initialize(&self, session: Option<&CachingSession>) -> MeowithIoResult<()> {
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
        let chunks_with_missing_meta = self.scan_fragments().await?;
        if !chunks_with_missing_meta.is_empty() {
            warn!(
                "{} chunks have missing metadata",
                chunks_with_missing_meta.len()
            );
            if let Some(session) = session {
                self.scan_missing_fragment_metadata(session, chunks_with_missing_meta)
                    .await?;
            }
        }

        Ok(())
    }

    async fn scan_fragments(&self) -> MeowithIoResult<HashSet<Uuid>> {
        info!("Scanning fragments...");
        let chunk_dir = Path::new(&self._internal.root_path);
        let dir_scan = fs::read_dir(chunk_dir).map_err(MeowithIoError::from)?;
        let mut chunk_map = self._internal.chunk_set.write().await;
        let mut last_notify = Instant::now();
        let ext_metadata_store_guard = self._internal.ext_metadata_store.read().await;
        let ext_metadata_store = ext_metadata_store_guard.as_ref().unwrap();
        let mut chunks_with_missing_meta = HashSet::new();

        for entry in dir_scan {
            let entry = entry.map_err(MeowithIoError::from)?;
            let entry_path = entry.path();
            let path = Path::new(&entry_path);
            if let Some(ext) = path.extension() {
                if ext == "uncommited" {
                    tokio::fs::remove_file(path).await?;
                    continue;
                }
            }
            match Uuid::from_str(entry.file_name().to_str().unwrap_or("invalid_unicode")) {
                Ok(id) => {
                    if let Ok(metadata) = entry.metadata() {
                        let discovered_chunk = FragmentMeta {
                            disk_content_size: metadata.len(),
                            disk_physical_size: path
                                .size_on_disk_fast(&metadata)
                                .unwrap_or(metadata.len()),
                        };
                        self._internal
                            .disk_content_size
                            .fetch_add(discovered_chunk.disk_content_size, Ordering::SeqCst);
                        self._internal
                            .disk_physical_size
                            .fetch_add(discovered_chunk.disk_physical_size, Ordering::SeqCst);
                        let associated_extra_metadata = ext_metadata_store.get(&id);
                        match associated_extra_metadata {
                            Ok(_) => {
                                chunk_map.insert(id, discovered_chunk);
                            }
                            Err(MeowithIoError::NotFound) => {
                                warn!("No associated extra file metadata found for {}", id);
                                chunks_with_missing_meta.insert(id);
                                chunk_map.insert(id, discovered_chunk);
                            }
                            Err(err) => {
                                panic!("fragment ledger init error {}", err);
                            }
                        }

                        if last_notify.elapsed() > Duration::from_secs(5) {
                            info!("Scanned {} entries so far", chunk_map.len());
                            last_notify = Instant::now();
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

        Ok(chunks_with_missing_meta)
    }

    /// For now, this is handled by #scan_missing_fragment_metadata
    /// during boot.
    /// Might be worth-while calling this from time to time during runtime.
    #[allow(dead_code)]
    pub async fn remove_orphaned_fragments(&self, session: &CachingSession) -> MeowithIoResult<()> {
        info!("Scanning for orphaned fragments...");
        let chunk_map = self._internal.chunk_set.read().await;
        let ext_meta = self._internal.ext_metadata_store.read().await;
        let ext_metadata_store = ext_meta.as_ref().unwrap();

        let mut mark = vec![];

        for id in (*chunk_map).keys() {
            let assoc_meta = ext_metadata_store.get(id);
            match assoc_meta {
                Ok(assoc) => {
                    let file = maybe_get_file_by_id(assoc.bucket_id(), assoc.file_id(), session)
                        .await
                        .map_err(|e| MeowithIoError::Internal(Some(Box::new(e))))?;

                    if file.is_none() {
                        warn!("No associated file found for {}", id);
                        mark.push(*id);
                    }
                }
                Err(MeowithIoError::NotFound) => {
                    warn!("No associated metadata for {}", id);
                }
                Err(err) => return Err(err),
            }
        }

        for sweep in mark {
            self.delete_chunk(&sweep).await?;
        }

        Ok(())
    }

    /// Attempt to find metadata for fragments.
    /// If no metadata is found, the fragment is assumed to be orphaned
    /// and will be removed.
    pub async fn scan_missing_fragment_metadata(
        &self,
        session: &CachingSession,
        mut missing_ids: HashSet<Uuid>,
    ) -> MeowithIoResult<()> {
        info!(
            "Scanning for missing fragment metadata, Missing: {}",
            missing_ids.len()
        );

        let mut file_stream = get_all_files(session)
            .await
            .map_err(|e| MeowithIoError::Internal(Some(Box::new(e))))?
            .into_stream();

        while let Some(file) = file_stream.next().await {
            let file = file.map_err(|e| MeowithIoError::Internal(Some(Box::new(e))))?;

            if missing_ids.contains(&file.id) {
                info!("Found missing fragment metadata for {}", file.id);
                self._internal
                    .ext_metadata_store
                    .read()
                    .await
                    .as_ref()
                    .map(|x| {
                        x.insert(
                            file.id,
                            ExtFragmentMeta {
                                bucket_id: file.bucket_id.to_u128_le(),
                                file_id: file.id.to_u128_le(),
                            },
                        )
                    });
                missing_ids.remove(&file.id);
            }
        }

        info!(
            "Orphaned fragments after scan {}. Deleting.",
            missing_ids.len()
        );
        for chunk_id in &missing_ids {
            self.delete_chunk(chunk_id).await?;
        }

        Ok(())
    }

    pub async fn get_storage_info(&self) -> UpdateStorageNodeProperties {
        let max = self._internal.max_physical_size.load(ORDERING_MAX_LOAD);
        UpdateStorageNodeProperties {
            max_space: max,
            used_space: max - self.get_available_space(),
            reserved: self._internal.reservation_map.read().await.len() as u64,
            commited: self._internal.chunk_set.read().await.len() as u64,
            uncommitted: self._internal.uncommited_map.read().await.len() as u64,
            paused: self._internal.paused.load(ORDERING_MAX_LOAD),
        }
    }

    #[inline(always)]
    pub fn lock_table(&self) -> LockTable {
        self._internal.file_lock_table.clone()
    }

    pub async fn stat_chunk(&self, chunk_id: &Uuid) -> MeowithIoResult<u64> {
        let uncommited = self._internal.uncommited_map.read().await;
        let uncommited = uncommited.contains_key(chunk_id);
        let path = self.get_path(chunk_id, uncommited);
        let file = File::open(path).await?;
        Ok(file.metadata().await?.len())
    }

    #[inline(always)]
    pub async fn fragment_exists(&self, chunk_id: &Uuid) -> bool {
        self._internal.chunk_set.read().await.contains_key(chunk_id)
    }

    pub async fn extended_fragment_meta(&self, chunk_id: &Uuid) -> Option<ExtFragmentMeta> {
        self._internal
            .ext_metadata_store
            .read()
            .await
            .as_ref()
            .and_then(|x| x.get(chunk_id).ok())
    }

    pub async fn existing_or_reserved_fragment_meta(
        &self,
        chunk_id: &Uuid,
    ) -> Option<FragmentMeta> {
        if let Some(reserved) = self._internal.reservation_map.read().await.get(chunk_id) {
            return Some(FragmentMeta {
                disk_content_size: reserved.completed,
                disk_physical_size: 0,
            });
        }

        self.existing_fragment_meta(chunk_id).await
    }

    #[inline(always)]
    pub async fn existing_fragment_meta(&self, chunk_id: &Uuid) -> Option<FragmentMeta> {
        self._internal.chunk_set.read().await.get(chunk_id).cloned()
    }

    #[inline(always)]
    fn get_path(&self, chunk_id: &Uuid, uncommited: bool) -> PathBuf {
        self._internal.get_path(chunk_id, uncommited)
    }

    #[inline(always)]
    pub async fn delete_chunk(&self, chunk_id: &Uuid) -> MeowithIoResult<()> {
        self._internal.delete_chunk(chunk_id).await
    }

    pub async fn fragment_read_stream(
        &self,
        chunk_id: &Uuid,
    ) -> MeowithIoResult<FragmentReadStream> {
        let file = File::open(self.get_path(chunk_id, false))
            .await
            .map_err(MeowithIoError::from)?;
        Ok(Arc::new(Mutex::new(Box::pin(BufReader::new(file)))))
    }

    pub async fn raw_fragment_read_omni_stream(
        &self,
        chunk_id: &Uuid,
    ) -> MeowithIoResult<BufStream<File>> {
        let file = File::open(self.get_path(chunk_id, false))
            .await
            .map_err(MeowithIoError::from)?;
        Ok(BufStream::new(file))
    }

    pub async fn fragment_read_omni_stream(
        &self,
        chunk_id: &Uuid,
    ) -> MeowithIoResult<FragmentReadOmniStream> {
        Ok(Arc::new(Mutex::new(Box::pin(
            self.raw_fragment_read_omni_stream(chunk_id).await?,
        ))))
    }

    pub async fn fragment_write_stream(
        &self,
        chunk_id: &Uuid,
    ) -> MeowithIoResult<FragmentWriteStream> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(self.get_path(chunk_id, true))
            .await
            .map_err(MeowithIoError::from)?;
        Ok(Arc::new(Mutex::new(Box::pin(BufWriter::new(file)))))
    }

    pub async fn fragment_append_stream(
        &self,
        chunk_id: &Uuid,
    ) -> MeowithIoResult<FragmentWriteStream> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(self.get_path(chunk_id, true))
            .await
            .map_err(MeowithIoError::from)?;
        Ok(Arc::new(Mutex::new(Box::pin(BufWriter::new(file)))))
    }

    /// Remove a reservation instantly.
    /// Used when a client reserves space on multiple nodes, but not all the calls succeed.
    /// In that case, the client cancels every reservation it has made up to that point.
    pub async fn cancel_reservation(&self, id: &Uuid) -> MeowithIoResult<()> {
        trace!("Fragment ledger Cancelling reservation {id}");
        let mut reservations = self._internal.reservation_map.write().await;
        if let Some(reservation) = reservations.remove(id) {
            let mut uncommited = self._internal.uncommited_map.write().await;
            let uncommited = uncommited.remove(id);
            let path = &self.get_path(id, uncommited.is_some());
            let _ = self
                ._internal
                .ext_metadata_store
                .read()
                .await
                .as_ref()
                .map(|x| x.remove(id));
            self._internal
                .disk_reserved_size
                .fetch_sub(reservation.file_space, ORDERING_DISK_STORE);
            tokio::fs::remove_file(path)
                .await
                .map_err(|_| MeowithIoError::Internal(None))?;
        }
        Ok(())
    }

    pub async fn try_reserve(
        &self,
        size: u64,
        associated_bucket_id: Uuid,
        associated_file_id: Uuid,
        durable: bool,
    ) -> MeowithIoResult<Uuid> {
        let paused = self._internal.paused.load(Ordering::Relaxed);
        if paused {
            return Err(MeowithIoError::Paused);
        }

        let mut reservations = self._internal.reservation_map.write().await;

        let available = self.get_available_space();
        trace!("Fragment ledger Try Reserve size={size} durable={durable} available={available}");
        if available < size {
            return Err(MeowithIoError::InsufficientDiskSpace);
        }
        let id = Uuid::new_v4();
        self._internal
            .ext_metadata_store
            .read()
            .await
            .as_ref()
            .map(|x| {
                x.insert(
                    id,
                    ExtFragmentMeta {
                        bucket_id: associated_bucket_id.to_u128_le(),
                        file_id: associated_file_id.to_u128_le(),
                    },
                )
            })
            .ok_or(MeowithIoError::Internal(None))??;

        let reservation = Reservation {
            file_space: size,
            completed: 0,
            durable,
            last_update: Instant::now(),
        };

        reservations.insert(id, reservation);

        self._internal
            .disk_reserved_size
            .fetch_add(size, ORDERING_DISK_STORE);

        let mut uncommited = self._internal.uncommited_map.write().await;
        uncommited.insert(id, CommitInfo::new());
        trace!("Fragment ledger Inserted uncommited chunk {id}");

        Ok(id)
    }

    /// Notifies the ledger that upload has been resumed, moving the chunk out of the broken queue.
    pub async fn resume_reservation(&self, id: &Uuid) -> MeowithIoResult<Reservation> {
        trace!("Fragment ledger Resuming reservation {id}");
        self.refresh_reservation(id).await
    }

    /// Moves the reservation into the idle 1H timeout state.
    pub async fn pause_reservation(&self, id: &Uuid) -> MeowithIoResult<()> {
        trace!("Fragment ledger pausing reservation {id}");
        self.refresh_reservation(id).await.map(|_| ())
    }

    pub async fn refresh_reservation(&self, id: &Uuid) -> MeowithIoResult<Reservation> {
        let mut reservations = self._internal.reservation_map.write().await;
        let reservation = reservations.get_mut(id).ok_or(MeowithIoError::NotFound)?;
        reservation.last_update = Instant::now();

        Ok(reservation.clone())
    }

    /// Drops the reservation.
    /// If the actual uploaded size of the chunk does not equal the expected size,
    /// the behavior depends on the durability of the upload.
    ///
    /// If the upload is durable, the chunk gets put into the broken queue,
    /// where it awaits further data for at most an hour.
    /// If it is not, the chunk is immediately dropped, releasing the reservation.
    pub async fn release_reservation(&self, id: &Uuid, size_actual: u64) -> MeowithIoResult<()> {
        trace!("Fragment ledger releasing reservation {id} {size_actual}");
        let mut reservations = self._internal.reservation_map.write().await;

        let reservation = reservations.get(id);
        if reservation.is_none() {
            return Err(MeowithIoError::NotFound);
        }
        let reservation = reservation.unwrap();

        let transfer_completed = size_actual == reservation.file_space;
        let expected = reservation.file_space;
        let mut uncommited = self._internal.uncommited_map.write().await;
        let is_uncommited = uncommited.contains_key(id);
        let path = &self.get_path(id, is_uncommited);

        trace!(
            "Fragment ledger releasing reservation completed: {transfer_completed} durable: {}",
            reservation.durable
        );
        if transfer_completed {
            let physical_size = Path::new(path)
                .size_on_disk()
                .map_err(|_| MeowithIoError::Internal(None))?;

            self._internal
                .disk_physical_size
                .fetch_add(physical_size, ORDERING_DISK_STORE);
            self._internal
                .disk_content_size
                .fetch_add(size_actual, ORDERING_DISK_STORE);
            reservations.remove(id);
            self._internal
                .disk_reserved_size
                .fetch_sub(size_actual, ORDERING_DISK_STORE);
            drop(reservations);
            let mut chunks = self._internal.chunk_set.write().await;
            chunks.insert(
                *id,
                FragmentMeta {
                    disk_content_size: size_actual,
                    disk_physical_size: physical_size,
                },
            );
            trace!("Fragment ledger Transfer finished");
        } else if !transfer_completed && reservation.durable {
            trace!(
                "Fragment ledger Durable transfer interrupted @ {size_actual} / {}",
                reservation.file_space
            );
            let reservation = reservations.get_mut(id).unwrap();
            reservation.completed = size_actual;
            reservation.last_update = Instant::now();
        } else if !transfer_completed && !reservation.durable {
            trace!("Non durable upload failure");
            reservations.remove(id);
            uncommited.remove(id);
            self._internal
                .disk_reserved_size
                .fetch_sub(expected, ORDERING_DISK_STORE);
            tokio::fs::remove_file(path)
                .await
                .map_err(|_| MeowithIoError::Internal(None))?;
        }

        Ok(())
    }

    pub async fn commit_chunk(&self, id: &Uuid) -> MeowithIoResult<()> {
        trace!("Fragment ledger Committing chunk {id}");
        let mut uncommited = self._internal.uncommited_map.write().await;
        let uncommited = uncommited.remove(id);
        if uncommited.is_some() {
            trace!("Fragment ledger Committing chunk {id} ok");
            tokio::fs::rename(self.get_path(id, true), self.get_path(id, false))
                .await
                .map_err(|_| MeowithIoError::Internal(None))?;
        } else {
            trace!("Fragment ledger Committing chunk {id} NotFound");
        }
        Ok(())
    }

    /// Update the timeout on the chunk.
    pub(crate) async fn commit_alive(&self, chunk_id: &Uuid) -> MeowithIoResult<()> {
        trace!("Fragment ledger commit alive {chunk_id}");
        let reservation = self.refresh_reservation(chunk_id).await;
        let uncommitted = match self._internal.uncommited_map.write().await.entry(*chunk_id) {
            Entry::Occupied(mut entry) => {
                entry.insert(CommitInfo::new());
                Ok(())
            }
            Entry::Vacant(_) => Err(MeowithIoError::NotFound),
        };

        if reservation.is_err() && uncommitted.is_err() {
            Err(MeowithIoError::NotFound)
        } else {
            Ok(())
        }
    }

    pub fn get_available_space(&self) -> u64 {
        let reserved = self._internal.disk_reserved_size.load(ORDERING_DISK_LOAD);
        let used = self._internal.disk_physical_size.load(ORDERING_DISK_LOAD) + reserved;
        let current = self._internal.max_physical_size.load(ORDERING_MAX_LOAD);

        trace!(
            "Node available space reserved: {}, used + reserved: {}, current: {}",
            reserved,
            used,
            current
        );
        current.saturating_sub(used)
    }
}

#[derive(Clone, Debug)]
pub struct Reservation {
    pub file_space: u64,
    pub completed: u64,
    pub durable: bool,
    pub last_update: Instant,
}

#[derive(Clone, Debug)]
pub struct FragmentMeta {
    pub disk_content_size: u64,
    pub disk_physical_size: u64,
}

#[derive(Clone, Debug)]
struct CommitInfo {
    pub access: Instant,
}

impl CommitInfo {
    fn new() -> Self {
        CommitInfo {
            access: Instant::now(),
        }
    }
}

struct InternalLedger {
    root_path: PathBuf,
    file_lock_table: LockTable,
    chunk_set: RwLock<HashMap<Uuid, FragmentMeta>>,
    reservation_map: RwLock<HashMap<Uuid, Reservation>>,
    uncommited_map: RwLock<HashMap<Uuid, CommitInfo>>,
    ext_metadata_store: RwLock<Option<Box<dyn ExtFragmentMetaStore>>>,

    housekeeper_handle: std::sync::Mutex<Option<JoinHandle<()>>>,

    max_physical_size: AtomicU64,
    disk_physical_size: AtomicU64,
    disk_content_size: AtomicU64,

    disk_reserved_size: AtomicU64,
    paused: AtomicBool,
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

    async fn clean_broken_chunks(&self) -> MeowithIoResult<()> {
        let mut broken = self.reservation_map.write().await;
        let mut uncommitted = self.uncommited_map.write().await;
        let mut mark = vec![];
        let max = Duration::from_secs(DURABLE_UPLOAD_SESSION_VALIDITY_TIME_SECS as u64);

        for (id, reservation) in &*broken {
            if reservation.last_update.elapsed() > max {
                mark.push(*id);
            }
        }

        if !mark.is_empty() {
            info!("Sweeping {} broken chunks", mark.len());
        }

        for sweep in mark {
            let uncommited = uncommitted.remove(&sweep).is_some();
            let path = &self.get_path(&sweep, uncommited);
            let reservation = broken.remove(&sweep).unwrap();
            let _ = self
                .ext_metadata_store
                .read()
                .await
                .as_ref()
                .map(|x| x.remove(&sweep));
            tokio::fs::remove_file(path)
                .await
                .map_err(|_| MeowithIoError::Internal(None))?;
            self.disk_reserved_size
                .fetch_sub(reservation.file_space, ORDERING_DISK_STORE);
        }

        Ok(())
    }

    async fn clean_uncommitted(&self) -> MeowithIoResult<()> {
        let mut mark = vec![];
        let max = Duration::from_secs(DURABLE_UPLOAD_SESSION_VALIDITY_TIME_SECS as u64);

        {
            let uncommitted = self.uncommited_map.read().await;
            for (id, info) in &*uncommitted {
                if info.access.elapsed() > max {
                    mark.push(*id);
                }
            }
        }

        if !mark.is_empty() {
            info!("Sweeping {} uncommitted chunks", mark.len());
        }

        for sweep in mark {
            self.delete_chunk(&sweep).await?
        }

        Ok(())
    }

    pub async fn delete_chunk(&self, chunk_id: &Uuid) -> MeowithIoResult<()> {
        let mut uncommited = self.uncommited_map.write().await;
        let _ = self
            .ext_metadata_store
            .read()
            .await
            .as_ref()
            .map(|x| x.remove(chunk_id));
        let uncommited = uncommited.remove(chunk_id);
        let path = self.get_path(chunk_id, uncommited.is_some());
        tokio::fs::remove_file(path)
            .await
            .map_err(|_| MeowithIoError::NotFound)?;

        if let Some(chunk) = self.chunk_set.write().await.remove(chunk_id) {
            self.disk_content_size
                .fetch_sub(chunk.disk_content_size, ORDERING_DISK_STORE);
            self.disk_physical_size
                .fetch_sub(chunk.disk_physical_size, ORDERING_DISK_STORE);
        } else if let Some(broken) = self.reservation_map.write().await.remove(chunk_id) {
            self.disk_reserved_size
                .fetch_sub(broken.file_space, ORDERING_DISK_STORE);
        }

        Ok(())
    }

    fn get_path(&self, chunk_id: &Uuid, uncommited: bool) -> PathBuf {
        if uncommited {
            self.get_path_uncommited(chunk_id)
        } else {
            let mut path = self.root_path.clone();
            path.push(chunk_id.to_string());
            path
        }
    }

    fn get_path_uncommited(&self, chunk_id: &Uuid) -> PathBuf {
        let mut path = self.root_path.clone();
        path.push(format!("{chunk_id}.uncommited"));
        path
    }
}

impl Drop for InternalLedger {
    fn drop(&mut self) {
        trace!("Dropping the internal ledger");
    }
}

impl Drop for FragmentLedger {
    fn drop(&mut self) {
        let refs = Arc::strong_count(&self._internal);
        trace!("Dropping the fragment ledger refs={refs}");
        if refs > 1 {
            return;
        }
        let a = self._internal.housekeeper_handle.lock().unwrap();
        match &*a {
            None => {}
            Some(handle) => {
                handle.abort();
            }
        }
    }
}
