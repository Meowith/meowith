#[cfg(test)]
mod tests {
    use uuid::Uuid;

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
            assert_eq!(guard_3.unwrap_err(), FileLockError::LockTaken)
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
            assert_eq!(guard_2.unwrap_err(), FileLockError::LockTaken)
        }

        {
            let id = Uuid::new_v4();
            let guard_1 = table.try_write(id).await;
            assert!(guard_1.is_ok());
            drop(guard_1);
            let guard_2 = table.try_read(id).await;
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
}
