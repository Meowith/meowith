use actix_web::{
    error::ResponseError,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use charybdis::errors::CharybdisError;
use derive_more::Display;
use scylla::cql_to_rust::FromRowError;
use scylla::transport::errors::QueryError;
use scylla::transport::iterator::NextRowError;

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
    QueryError(QueryError),
    InternalFailure(CharybdisError),
    FromRowError(FromRowError),
    NextRowError(NextRowError),
    /// Used when a LWT couldn't update the record
    LockingError,
    NotFound,
    UnknownFailure,
}

impl From<CharybdisError> for MeowithDataError {
    fn from(value: CharybdisError) -> Self {
        match value {
            CharybdisError::NotFoundError(_) => MeowithDataError::NotFound,
            _ => MeowithDataError::InternalFailure(value),
        }
    }
}

impl From<NextRowError> for MeowithDataError {
    fn from(value: NextRowError) -> Self {
        MeowithDataError::NextRowError(value)
    }
}

impl From<QueryError> for MeowithDataError {
    fn from(value: QueryError) -> Self {
        MeowithDataError::QueryError(value)
    }
}
