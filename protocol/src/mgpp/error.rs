use openssl::error::ErrorStack;
use std::array::TryFromSliceError;
use std::error::Error;

pub type MGPPResult<T> = Result<T, MGPPError>;

#[derive(Debug, derive_more::Display)]
pub enum MGPPError {
    ConnectionError,
    #[display("SSLError {_0:?}")]
    SSLError(Option<Box<dyn Error + Send + Sync>>),
    AddressResolutionError,
    ConnectionAuthenticationError,
    ShuttingDown,
}
macro_rules! impl_ssl_from_error {
    ($error_type:ty) => {
        impl From<$error_type> for MGPPError {
            fn from(error: $error_type) -> Self {
                MGPPError::SSLError(Some(Box::new(error)))
            }
        }
    };
}

impl_ssl_from_error!(ErrorStack);
impl_ssl_from_error!(rustls::Error);
impl_ssl_from_error!(std::io::Error);

impl From<TryFromSliceError> for MGPPError {
    fn from(_: TryFromSliceError) -> Self {
        MGPPError::ConnectionError
    }
}

impl Error for MGPPError {}
