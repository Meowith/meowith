use std::sync::{Arc, Weak};

use tokio::sync::{OwnedSemaphorePermit, TryAcquireError};

use crate::locking::error::FileLockError;
use crate::locking::file_lock_table::{KeyBounds, Locker, TryLockResult};

#[allow(unused)]
#[derive(Debug)]
pub struct FileReadGuard<K: KeyBounds> {
    pub(super) locker: Weak<Locker<K>>,
    pub(super) permit: OwnedSemaphorePermit,
}

impl<K: KeyBounds> FileReadGuard<K> {
    pub(crate) fn new(
        locker: Arc<Locker<K>>,
    ) -> TryLockResult<FileReadGuard<K>> {
        let permit = match Arc::clone(&locker.semaphore).try_acquire_owned() {
            Ok(permit) => Ok(permit),
            Err(TryAcquireError::NoPermits) => Err(FileLockError::LockTaken),
            Err(TryAcquireError::Closed) => unreachable!(), // The semaphore is never explicitly closed.
        }?;

        Ok(FileReadGuard {
            locker: Arc::downgrade(&locker),
            permit,
        })
    }
}

impl<K: KeyBounds> Drop for FileReadGuard<K> {
    #[allow(dead_code)]
    fn drop(&mut self) {
        if let Some(locker) = self.locker.upgrade() {
            locker.release_read()
        }
    }
}
