use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::{error, HttpResponse};
use derive_more::Display;
use log::error;
use serde::Serialize;

use crate::error::io_error::MeowithIoError;
use crate::error::mdsftp_error::MDSFTPError;
use data::error::MeowithDataError;

pub type NodeClientResponse<T> = Result<T, NodeClientError>;

#[derive(Serialize)]
pub(crate) struct ErrorResponse {
    pub(crate) message: String,
}

#[derive(Debug, Display)]
pub enum NodeClientError {
    InternalError,
    BadRequest,
    NotFound,
    EntityExists,
    NoSuchSession,
    BadAuth,
    InsufficientStorage,
    NotEmpty,
}

impl error::ResponseError for NodeClientError {
    fn status_code(&self) -> StatusCode {
        match *self {
            NodeClientError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            NodeClientError::BadRequest => StatusCode::BAD_REQUEST,
            NodeClientError::BadAuth => StatusCode::UNAUTHORIZED,
            NodeClientError::InsufficientStorage => StatusCode::IM_A_TEAPOT,
            NodeClientError::NotFound => StatusCode::NOT_FOUND,
            NodeClientError::NoSuchSession => StatusCode::NOT_FOUND,
            NodeClientError::EntityExists => StatusCode::BAD_REQUEST,
            NodeClientError::NotEmpty => StatusCode::BAD_REQUEST,
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

impl From<MeowithDataError> for NodeClientError {
    fn from(value: MeowithDataError) -> Self {
        match value {
            MeowithDataError::NotFound => NodeClientError::NotFound,
            _ => {
                error!("DB ERROR: {:?}", value);
                NodeClientError::InternalError
            }
        }
    }
}

impl From<MeowithIoError> for NodeClientError {
    fn from(value: MeowithIoError) -> Self {
        match value {
            MeowithIoError::NotFound => NodeClientError::NotFound,
            _ => {
                error!("MEOWITH IO ERROR: {:?}", value);
                NodeClientError::InternalError
            }
        }
    }
}

impl From<MDSFTPError> for NodeClientError {
    fn from(value: MDSFTPError) -> Self {
        match value {
            MDSFTPError::ReserveError(_) => NodeClientError::InsufficientStorage,
            _ => {
                error!("MDSFTP ERROR: {:?}", value);
                NodeClientError::InternalError
            }
        }
    }
}
