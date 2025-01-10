use std::fmt::Debug;
use std::io;
use std::io::Error;

pub type ProtocolResult<T> = Result<T, ProtocolError>;

/// Errors that can occur in the protocol
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Stream is closing")]
    ShuttingDown,
    #[error("Connection error")]
    ConnectionError,
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Failed to write packet into the stream")]
    WriteError(Error),
    #[error("Invalid packet format")]
    InvalidFormat,
    #[error("Unexpected packet size")]
    SizeMismatch,
    #[error("Failed to read packet from stream")]
    ReadError(Error),
    #[error("Uuid error occurred")]
    Uuid(#[from] uuid::Error),
    #[error("An error occurred")]
    Ambiguous(#[from] Error),
    #[error("An error occurred {0}")]
    Custom(String),
}

impl From<ProtocolError> for Error {
    fn from(value: ProtocolError) -> Self {
        Error::new(io::ErrorKind::InvalidInput, format!("{:?}", value))
    }
}
impl From<Box<dyn std::error::Error>> for ProtocolError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        ProtocolError::Custom(value.to_string())
    }
}
