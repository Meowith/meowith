use std::io;
use std::io::Error;

/// Errors that can occur in the protocol
#[derive(Debug)]
pub enum ProtocolError {
    ShuttingDown,
    ConnectionError,
    AuthenticationFailed,
    Custom(String),
}

impl From<ProtocolError> for Error {
    fn from(value: ProtocolError) -> Self {
        Error::new(io::ErrorKind::InvalidInput, format!("{:?}", value))
    }
}