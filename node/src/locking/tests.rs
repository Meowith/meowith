#[cfg(test)]
mod lock_table_tests {
    use uuid::Uuid;
    use tokio::time::{timeout, Duration};

    use crate::locking::error::FileLockError;
    use crate::locking::file_lock_table::FileLockTable;

    #[tokio::test]
    async fn test_read() {
        let table: FileLockTable<Uuid> = FileLockTable::new(2);

        {
            let id = Uuid::new_v4();
            let guard_1 = table.try_read(id).await;
            assert!(guard_1.is_ok());
            let guard_2 = table.try_read(id).await;
            assert!(guard_2.is_ok());
            let guard_3 = table.try_read(id).await;
            assert!(guard_3.is_err());
            assert_eq!(guard_3.unwrap_err(), FileLockError::LockTaken);

            drop(guard_1);
            let guard_4 = table.read(id).await;
            assert!(guard_4.is_ok());
        }
    }

    #[tokio::test]
    async fn test_write() {
        let table: FileLockTable<Uuid> = FileLockTable::new(2);

        {
            let id = Uuid::new_v4();
            let guard_1 = table.try_write(id).await;
            assert!(guard_1.is_ok());
            let guard_2 = table.try_read(id).await;
            assert!(guard_2.is_err());
            assert_eq!(guard_2.unwrap_err(), FileLockError::LockTaken);
        }

        {
            let id = Uuid::new_v4();
            let guard_1 = table.try_write(id).await;
            assert!(guard_1.is_ok());
            drop(guard_1);
            let guard_2 = table.read(id).await;
            assert!(guard_2.is_ok());
        }

        {
            let id1 = Uuid::new_v4();
            let id2 = Uuid::new_v4();
            let guard_1 = table.try_write(id1).await;
            assert!(guard_1.is_ok());
            let guard_2 = table.try_write(id2).await;
            assert!(guard_2.is_ok());
        }
    }

    #[tokio::test]
    async fn test_blocking_read_and_write() {
        let table: FileLockTable<Uuid> = FileLockTable::new(1);

        let id = Uuid::new_v4();

        {
            let guard_1 = table.try_write(id).await;
            assert!(guard_1.is_ok());

            let handle = tokio::spawn({
                let table = table.clone();
                async move { table.read(id).await }
            });

            // Simulate blocking until write lock is released
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(guard_1);

            let guard_2 = timeout(Duration::from_secs(1), handle).await.unwrap().unwrap();
            assert!(guard_2.is_ok());
        }

        {
            let guard_1 = table.try_read(id).await;
            assert!(guard_1.is_ok());

            let handle = tokio::spawn({
                let table = table.clone();
                async move { table.write(id).await }
            });

            // Simulate blocking until read lock is released
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(guard_1);

            let guard_2 = timeout(Duration::from_secs(1), handle).await.unwrap().unwrap();
            assert!(guard_2.is_ok());
        }
    }
}
