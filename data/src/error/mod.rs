use charybdis::errors::CharybdisError;
use strum::Display;

#[derive(Debug, Display)]
pub enum MeowithDataError {
    InternalFailure(CharybdisError),
    NotFound,
}

impl From<CharybdisError> for MeowithDataError {
    fn from(value: CharybdisError) -> Self {
        match value {
            CharybdisError::NotFoundError(_) => MeowithDataError::NotFound,
            _ => MeowithDataError::InternalFailure(value),
        }
    }
}
