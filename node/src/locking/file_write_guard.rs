use std::sync::{Arc, Weak};

use tokio::sync::{OwnedSemaphorePermit, TryAcquireError};

use crate::locking::error::FileLockError;
use crate::locking::file_lock_table::{KeyBounds, Locker, TryLockResult};

#[derive(Debug)]
pub struct FileWriteGuard<K: KeyBounds> {
    pub(super) locker: Weak<Locker<K>>,
    #[allow(unused)] // this is necessary
    pub(super) permit: OwnedSemaphorePermit,
}

impl<K: KeyBounds> FileWriteGuard<K> {
    pub(crate) async fn new(
        locker: Arc<Locker<K>>,
        is_attempt: bool,
    ) -> TryLockResult<FileWriteGuard<K>> {
        let permit = if is_attempt {
            match Arc::clone(&locker.semaphore).try_acquire_many_owned(locker.max_readers) {
                Ok(permit) => Ok(permit),
                Err(TryAcquireError::NoPermits) => Err(FileLockError::LockTaken),
                Err(TryAcquireError::Closed) => unreachable!(), // The semaphore is never explicitly closed.
            }?
        } else {
            match Arc::clone(&locker.semaphore)
                .acquire_many_owned(locker.max_readers)
                .await
            {
                Ok(permit) => Ok(permit),
                Err(_) => unreachable!(), // The semaphore is never explicitly closed.
            }?
        };

        Ok(FileWriteGuard {
            locker: Arc::downgrade(&locker),
            permit,
        })
    }
}

impl<K: KeyBounds> Drop for FileWriteGuard<K> {
    fn drop(&mut self) {
        if let Some(locker) = self.locker.upgrade() {
            locker.release_write()
        }
    }
}
