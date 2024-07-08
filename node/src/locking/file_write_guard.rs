use std::marker::PhantomData;
use std::sync::{Arc, Weak};

use tokio::sync::{OwnedSemaphorePermit, TryAcquireError};

use crate::locking::error::FileLockError;
use crate::locking::file_lock_table::{KeyBounds, Locker, TryLockResult, ValueBounds};

#[allow(unused)]
#[derive(Debug)]
pub struct FileWriteGuard<'a, K: KeyBounds, T: ValueBounds<K>> {
    pub(super) locker: Weak<Locker<K, T>>,
    pub(super) permit: OwnedSemaphorePermit,
    pub(super) data: *const T,
    pub(super) marker: PhantomData<&'a T>,
}

impl<'a, K: KeyBounds, T: ValueBounds<K>> FileWriteGuard<'a, K, T> {
    pub(crate) fn new(
        locker: Arc<Locker<K, T>>,
        data: *const T,
    ) -> TryLockResult<FileWriteGuard<'a, K, T>> {
        let permit = match Arc::clone(&locker.semaphore).try_acquire_many_owned(locker.max_readers)
        {
            Ok(permit) => Ok(permit),
            Err(TryAcquireError::NoPermits) => Err(FileLockError::LockTaken),
            Err(TryAcquireError::Closed) => unreachable!(), // The semaphore is never explicitly closed.
        }?;

        Ok(FileWriteGuard {
            locker: Arc::downgrade(&locker),
            permit,
            data,
            marker: PhantomData,
        })
    }
}

impl<'a, K: KeyBounds, T: ValueBounds<K>> Drop for FileWriteGuard<'a, K, T> {
    #[allow(dead_code)]
    fn drop(&mut self) {
        if let Some(locker) = self.locker.upgrade() {
            locker.release_write()
        }
    }
}
