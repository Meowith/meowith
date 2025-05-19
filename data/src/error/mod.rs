use actix_web::{
    error::ResponseError,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use charybdis::errors::CharybdisError;
use derive_more::Display;
use scylla::deserialize::{DeserializationError, TypeCheckError};
use scylla::errors::{
    ExecutionError, IntoRowsResultError, NextRowError, PagerExecutionError, RowsError,
};
use std::error::Error;

#[derive(Debug, Display)]
pub enum DataResponseError {
    #[display("bad auth")]
    BadAuth,
}
impl ResponseError for DataResponseError {
    fn status_code(&self) -> StatusCode {
        match *self {
            DataResponseError::BadAuth => StatusCode::UNAUTHORIZED,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::html())
            .body(self.to_string())
    }
}

#[derive(Debug, Display)]
pub enum MeowithDataError {
    QueryError(Box<dyn Error + Send + Sync>),
    InternalFailure(CharybdisError),
    /// Used when a LWT couldn't update the record
    LockingError,
    NotFound,
    UnknownFailure,
}

impl Error for MeowithDataError {}

impl From<CharybdisError> for MeowithDataError {
    fn from(value: CharybdisError) -> Self {
        match value {
            CharybdisError::NotFoundError(_) => MeowithDataError::NotFound,
            _ => MeowithDataError::InternalFailure(value),
        }
    }
}

impl From<TypeCheckError> for MeowithDataError {
    fn from(value: TypeCheckError) -> Self {
        MeowithDataError::QueryError(Box::new(value))
    }
}

impl From<DeserializationError> for MeowithDataError {
    fn from(value: DeserializationError) -> Self {
        MeowithDataError::QueryError(Box::new(value))
    }
}

impl From<PagerExecutionError> for MeowithDataError {
    fn from(value: PagerExecutionError) -> Self {
        MeowithDataError::QueryError(Box::new(value))
    }
}

impl From<ExecutionError> for MeowithDataError {
    fn from(value: ExecutionError) -> Self {
        MeowithDataError::QueryError(Box::new(value))
    }
}

impl From<IntoRowsResultError> for MeowithDataError {
    fn from(value: IntoRowsResultError) -> Self {
        MeowithDataError::QueryError(Box::new(value))
    }
}

impl From<RowsError> for MeowithDataError {
    fn from(value: RowsError) -> Self {
        MeowithDataError::QueryError(Box::new(value))
    }
}

impl From<NextRowError> for MeowithDataError {
    fn from(value: NextRowError) -> Self {
        MeowithDataError::QueryError(Box::new(value))
    }
}
