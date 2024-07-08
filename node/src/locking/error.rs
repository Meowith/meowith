#[derive(Debug, Eq, PartialEq)]
pub enum FileLockError {
    LockTaken,
}
