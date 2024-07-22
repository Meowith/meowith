use std::array::TryFromSliceError;
use std::error::Error;
use openssl::error::ErrorStack;

#[allow(unused)]
pub type CatcheResult<T> = Result<T, CatcheError>;

#[derive(Debug, derive_more::Display)]
#[allow(unused)]
pub enum CatcheError {
    ConnectionError,
    #[display(fmt = "SSLError {_0:?}")]
    SSLError(Option<Box<dyn Error + Send + Sync>>),
    AddressResolutionError,
    ConnectionAuthenticationError,
    ShuttingDown,
}
macro_rules! impl_ssl_from_error {
    ($error_type:ty) => {
        impl From<$error_type> for CatcheError {
            fn from(error: $error_type) -> Self {
                CatcheError::SSLError(Some(Box::new(error)))
            }
        }
    };
}

impl_ssl_from_error!(ErrorStack);
impl_ssl_from_error!(rustls::Error);
impl_ssl_from_error!(std::io::Error);

impl From<TryFromSliceError> for CatcheError {
    fn from(_: TryFromSliceError) -> Self {
        CatcheError::ConnectionError
    }
}

impl Error for CatcheError {}
