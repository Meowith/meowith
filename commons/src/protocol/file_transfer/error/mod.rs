use openssl::error::ErrorStack;
use std::error::Error;

pub type MDSFTPResult<T> = Result<T, MDSFTPError>;

#[derive(Debug, derive_more::Display)]
pub enum MDSFTPError {
    ConnectionError,
    SSLError,
    NoSuchNode,
    AddressResolutionError,
}

impl From<ErrorStack> for MDSFTPError {
    fn from(_: ErrorStack) -> Self {
        MDSFTPError::SSLError
    }
}

impl Error for MDSFTPError {}
