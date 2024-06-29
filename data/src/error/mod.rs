use actix_web::{
    error::ResponseError,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use charybdis::errors::CharybdisError;
use derive_more::Display;
use scylla::transport::errors::QueryError;

#[derive(Debug, Display)]
pub enum DataResponseError {
    #[display(fmt = "bad auth")]
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

impl From<QueryError> for MeowithDataError {
    fn from(value: QueryError) -> Self {
        MeowithDataError::QueryError(value)
    }
}
