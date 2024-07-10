use openssl::error::ErrorStack;
use std::error::Error;

pub type MDSFTPResult<T> = Result<T, MDSFTPError>;

#[derive(Debug, derive_more::Display)]
pub enum MDSFTPError {
    ConnectionError,
    #[display(fmt = "SSLError {_0:?}")]
    SSLError(Option<Box<dyn Error + Send + Sync>>),
    NoSuchNode,
    AddressResolutionError,
    ConnectionAuthenticationError,

    NoSuchChunkId,
    #[display(fmt = "ReserveError max_space = {_0}")]
    ReserveError(u64),
    MaxChannels,
    Interrupted,
    ShuttingDown,
    RemoteError,
    NoPacketHandler,
    NoPool,
}

macro_rules! impl_ssl_from_error {
    ($error_type:ty) => {
        impl From<$error_type> for MDSFTPError {
            fn from(error: $error_type) -> Self {
                MDSFTPError::SSLError(Some(Box::new(error)))
            }
        }
    };
}

impl_ssl_from_error!(ErrorStack);
impl_ssl_from_error!(rustls::Error);
impl_ssl_from_error!(std::io::Error);

impl Error for MDSFTPError {}
