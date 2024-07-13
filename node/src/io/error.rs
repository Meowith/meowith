use std::error::Error;

pub type MeowithIoResult<T> = Result<T, MeowithIoError>;

#[derive(Debug, derive_more::Display)]
#[allow(unused)]
pub enum MeowithIoError {
    NotFound,
    #[display(fmt = "NotFound err = {_0:?}")]
    Internal(Option<Box<dyn Error + Send + Sync>>),
    InvalidDataDir,
    InsufficientDiskSpace,
}

impl Error for MeowithIoError {}

impl From<std::io::Error> for MeowithIoError {
    fn from(error: std::io::Error) -> Self {
        MeowithIoError::Internal(Some(Box::new(error)))
    }
}
