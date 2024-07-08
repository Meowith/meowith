use std::cell::UnsafeCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, Weak};

use tokio::sync::{Mutex, Semaphore};

use crate::locking::error::FileLockError;
use crate::locking::file_read_guard::FileReadGuard;
use crate::locking::file_write_guard::FileWriteGuard;
use crate::locking::KyeAbleValue;

pub type TryLockResult<T> = Result<T, FileLockError>;

pub(crate) type LockTable<K, T> = Mutex<HashMap<K, FileLock<K, T>>>;
pub trait KeyBounds: Sized + Eq + Hash + Clone + Send + Sync + Debug + 'static {}
impl<T> KeyBounds for T where T: Sized + Eq + Hash + Clone + Send + Sync + Debug + 'static {}

pub trait ValueBounds<K>: Sized + KyeAbleValue<K> + Send + 'static
where
    K: KeyBounds,
{
}
impl<T, K> ValueBounds<K> for T
where
    K: KeyBounds,
    T: Sized + KyeAbleValue<K> + Send + 'static,
{
}

pub const MAX_READERS: u32 = 256;

pub struct FileLockTable<K, T: Sized>
where
    K: KeyBounds,
    T: ValueBounds<K>,
{
    lock_table: Arc<LockTable<K, T>>,
    max_readers: u32,
}

impl<K: KeyBounds, T: ValueBounds<K>> Default for FileLockTable<K, T> {
    fn default() -> Self {
        FileLockTable::default()
    }
}

#[allow(unused)]
impl<K: KeyBounds, T: ValueBounds<K>> FileLockTable<K, T> {
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

    pub async fn try_read(&self, key: K) -> TryLockResult<FileReadGuard<'_, K, T>> {
        match self.lock_table.lock().await.entry(key.clone()) {
            Entry::Occupied(lock_entry) => lock_entry.get().try_read(),
            Entry::Vacant(entry) => {
                let data = T::new(&key);
                let lock = FileLock::new(
                    Arc::downgrade(&self.lock_table),
                    self.max_readers,
                    key,
                    data,
                );
                let guard = lock.try_read();
                entry.insert(lock);
                guard
            }
        }
    }

    pub async fn try_write(&self, key: K) -> TryLockResult<FileWriteGuard<'_, K, T>> {
        match self.lock_table.lock().await.entry(key.clone()) {
            Entry::Occupied(lock_entry) => lock_entry.get().try_write(),
            Entry::Vacant(entry) => {
                let data = T::new(&key);
                let lock = FileLock::new(
                    Arc::downgrade(&self.lock_table),
                    self.max_readers,
                    key,
                    data,
                );
                let guard = lock.try_write();
                entry.insert(lock);
                guard
            }
        }
    }
}

pub struct FileLock<K: KeyBounds, T: ValueBounds<K>> {
    locker: Arc<Locker<K, T>>,
    data: UnsafeCell<T>,
}

impl<K: KeyBounds, T: ValueBounds<K>> FileLock<K, T> {
    pub fn new(lock_table: Weak<LockTable<K, T>>, max_readers: u32, key: K, data: T) -> Self {
        FileLock {
            locker: Arc::new(Locker {
                max_readers,
                key,
                semaphore: Arc::new(Semaphore::new(max_readers as usize)),
                lock_table,
            }),
            data: UnsafeCell::new(data),
        }
    }

    pub fn try_read<'a>(&self) -> TryLockResult<FileReadGuard<'a, K, T>> {
        FileReadGuard::new(self.locker.clone(), self.data.get())
    }

    pub fn try_write<'a>(&self) -> TryLockResult<FileWriteGuard<'a, K, T>> {
        FileWriteGuard::new(self.locker.clone(), self.data.get())
    }
}

pub(crate) struct Locker<K: KeyBounds, T: ValueBounds<K>> {
    pub(crate) semaphore: Arc<Semaphore>,
    pub(crate) max_readers: u32,
    key: K,
    lock_table: Weak<LockTable<K, T>>,
}

#[allow(unused)]
impl<K: KeyBounds, T: ValueBounds<K>> Locker<K, T> {
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
