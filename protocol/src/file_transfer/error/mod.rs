use openssl::error::ErrorStack;
use std::error::Error;

pub type MDSFTPResult<T> = Result<T, MDSFTPError>;

#[derive(Debug, derive_more::Display)]
pub enum MDSFTPError {
    ConnectionError,
    SSLError,
    NoSuchNode,
    AddressResolutionError,

    NoSuchChunkId,
    #[display(fmt = "ReserveError max_space = {_0}")]
    ReserveError(u64),
    MaxChannels,
    Interrupted,
    RemoteError,
    NoPacketHandler,
    NoPool
}

impl From<ErrorStack> for MDSFTPError {
    fn from(_: ErrorStack) -> Self {
        MDSFTPError::SSLError
    }
}

impl Error for MDSFTPError {}
