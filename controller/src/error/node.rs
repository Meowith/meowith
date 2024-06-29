use actix_web::{
    error,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use derive_more::Display;

#[derive(Debug, Display)]
pub enum NodeError {
    #[display(fmt = "internal error")]
    InternalError,
    #[display(fmt = "bad request")]
    BadRequest,
    #[display(fmt = "bad auth")]
    BadAuth,
}

impl error::ResponseError for NodeError {
    fn status_code(&self) -> StatusCode {
        match *self {
            NodeError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            NodeError::BadRequest => StatusCode::BAD_REQUEST,
            NodeError::BadAuth => StatusCode::UNAUTHORIZED,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::html())
            .body(self.to_string())
    }
}
