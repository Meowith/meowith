use actix_web::{error, HttpResponse};
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use derive_more::Display;
use serde::Serialize;

pub type NodeClientResponse<T> = Result<T, NodeClientError>;

#[derive(Serialize)]
struct ErrorResponse {
    message: String
}

#[allow(unused)]
#[derive(Debug, Display)]
pub enum NodeClientError {
    InternalError,
    BadRequest,
    BadAuth,
    InsufficientStorage,
}

impl error::ResponseError for NodeClientError {
    fn status_code(&self) -> StatusCode {
        match *self {
            NodeClientError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            NodeClientError::BadRequest => StatusCode::BAD_REQUEST,
            NodeClientError::BadAuth => StatusCode::UNAUTHORIZED,
            NodeClientError::InsufficientStorage => StatusCode::IM_A_TEAPOT
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::json())
            .json(ErrorResponse {
                message: self.to_string(),
            })
    }
}
