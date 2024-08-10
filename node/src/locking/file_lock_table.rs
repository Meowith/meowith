use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, Weak};

use tokio::sync::{Mutex, Semaphore};

use crate::locking::error::FileLockError;
use crate::locking::file_read_guard::FileReadGuard;
use crate::locking::file_write_guard::FileWriteGuard;

pub type TryLockResult<T> = Result<T, FileLockError>;

pub(crate) type LockTable<K> = Mutex<HashMap<K, FileLock<K>>>;
pub trait KeyBounds: Sized + Eq + Hash + Clone + Send + Sync + Debug + 'static {}
impl<T> KeyBounds for T where T: Sized + Eq + Hash + Clone + Send + Sync + Debug + 'static {}

pub const MAX_READERS: u32 = 256;

#[derive(Clone)]
pub struct FileLockTable<K>
where
    K: KeyBounds,
{
    lock_table: Arc<LockTable<K>>,
    max_readers: u32,
}

impl<K: KeyBounds> Default for FileLockTable<K> {
    fn default() -> Self {
        FileLockTable::default()
    }
}

impl<K: KeyBounds> FileLockTable<K> {
    fn default() -> Self {
        FileLockTable {
            lock_table: Arc::new(Mutex::new(HashMap::new())),
            max_readers: MAX_READERS,
        }
    }

    pub fn new(max_readers: u32) -> Self {
        FileLockTable {
            lock_table: Arc::new(Mutex::new(HashMap::new())),
            max_readers,
        }
    }

    pub async fn try_read(&self, key: K) -> TryLockResult<FileReadGuard<K>> {
        match self.lock_table.lock().await.entry(key.clone()) {
            Entry::Occupied(lock_entry) => lock_entry.get().try_read(),
            Entry::Vacant(entry) => {
                let lock = FileLock::new(Arc::downgrade(&self.lock_table), self.max_readers, key);
                let guard = lock.try_read();
                entry.insert(lock);
                guard
            }
        }
    }

    pub async fn try_write(&self, key: K) -> TryLockResult<FileWriteGuard<K>> {
        match self.lock_table.lock().await.entry(key.clone()) {
            Entry::Occupied(lock_entry) => lock_entry.get().try_write(),
            Entry::Vacant(entry) => {
                let lock = FileLock::new(Arc::downgrade(&self.lock_table), self.max_readers, key);
                let guard = lock.try_write();
                entry.insert(lock);
                guard
            }
        }
    }
}

pub struct FileLock<K: KeyBounds> {
    locker: Arc<Locker<K>>,
}

impl<K: KeyBounds> FileLock<K> {
    pub fn new(lock_table: Weak<LockTable<K>>, max_readers: u32, key: K) -> Self {
        FileLock {
            locker: Arc::new(Locker {
                max_readers,
                key,
                semaphore: Arc::new(Semaphore::new(max_readers as usize)),
                lock_table,
            }),
        }
    }

    pub fn try_read(&self) -> TryLockResult<FileReadGuard<K>> {
        FileReadGuard::new(self.locker.clone())
    }

    pub fn try_write(&self) -> TryLockResult<FileWriteGuard<K>> {
        FileWriteGuard::new(self.locker.clone())
    }
}

pub(crate) struct Locker<K: KeyBounds> {
    pub(crate) semaphore: Arc<Semaphore>,
    pub(crate) max_readers: u32,
    key: K,
    lock_table: Weak<LockTable<K>>,
}

impl<K: KeyBounds> Locker<K> {
    pub(crate) fn release_read(&self) {
        if let Some(lock_table) = self.lock_table.upgrade() {
            // +1 as this method is called during the drop method, before the drop of the permit.
            if self.semaphore.available_permits() + 1 == self.max_readers as usize {
                // Lock no longer used, as no awaiting can be done on it as of right now.
                // Drop the table record.
                let k = self.key.clone();
                tokio::spawn(async move {
                    let mut table = lock_table.lock().await;
                    let _ = table.remove(&k);
                });
            }
        }
    }

    pub(crate) fn release_write(&self) {
        if let Some(lock_table) = self.lock_table.upgrade() {
            // Immediate release as the write lock holds all the permits.

            let k = self.key.clone();
            tokio::spawn(async move {
                let mut table = lock_table.lock().await;
                let _ = table.remove(&k);
            });
        }
    }
}
