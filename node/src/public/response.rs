use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::{error, HttpResponse};
use derive_more::Display;

pub type NodeClientResponse<T> = Result<T, NodeClientError>;

#[allow(unused)]
#[derive(Debug, Display)]
pub enum NodeClientError {
    #[display(fmt = "internal error")]
    InternalError,
    #[display(fmt = "bad request")]
    BadRequest,
    #[display(fmt = "bad auth")]
    BadAuth,
}

impl error::ResponseError for NodeClientError {
    fn status_code(&self) -> StatusCode {
        match *self {
            NodeClientError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            NodeClientError::BadRequest => StatusCode::BAD_REQUEST,
            NodeClientError::BadAuth => StatusCode::UNAUTHORIZED,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::html())
            .body(self.to_string())
    }
}
