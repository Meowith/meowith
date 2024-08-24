use openssl::error::ErrorStack;
use std::array::TryFromSliceError;
use std::error::Error;

pub type MDSFTPResult<T> = Result<T, MDSFTPError>;

#[derive(Debug, derive_more::Display)]
pub enum MDSFTPError {
    ConnectionError,
    #[display("SSLError {_0:?}")]
    SSLError(Option<Box<dyn Error + Send + Sync>>),
    NoSuchNode,
    AddressResolutionError,
    ConnectionAuthenticationError,

    NoSuchChunkId,
    #[display("ReserveError max_space = {_0}")]
    ReserveError(u64),
    ReservationError,
    MaxChannels,
    Interrupted,
    ShuttingDown,
    RemoteError,
    NoPacketHandler,
    NoPool,
    Internal,
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

impl From<TryFromSliceError> for MDSFTPError {
    fn from(_: TryFromSliceError) -> Self {
        MDSFTPError::ConnectionError
    }
}

impl Error for MDSFTPError {}
