use derive_more::Display;
use actix_web::{
    error,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};

#[derive(Debug, Display)]
pub enum NodeError {
    #[display(fmt = "internal error")]
    InternalError,
    #[display(fmt = "bad request")]
    BadRequest
}

impl error::ResponseError for NodeError {
    fn status_code(&self) -> StatusCode {
        match *self {
            NodeError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            NodeError::BadRequest => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::html())
            .body(self.to_string())
    }
}