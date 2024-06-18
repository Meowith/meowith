use charybdis::errors::CharybdisError;
use scylla::transport::errors::QueryError;
use strum::Display;

#[derive(Debug, Display)]
pub enum MeowithDataError {
    QueryError(QueryError),
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

impl From<QueryError> for MeowithDataError {
    fn from(value: QueryError) -> Self {
        MeowithDataError::QueryError(value)
    }
}
