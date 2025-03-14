use crate::error::io_error::MeowithIoError;
use crate::error::mdsftp_error::MDSFTPError;
use actix_web::error::PayloadError;
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;
use actix_web::{error, HttpResponse};
use bcrypt::BcryptError;
use data::error::MeowithDataError;
use derive_more::Display;
use jsonwebtoken::errors::Error;
use log::error;
use serde::Serialize;
use tokio::task::JoinError;

pub type NodeClientResponse<T> = Result<T, NodeClientError>;

#[derive(Clone, Debug, Display, Serialize)]
#[serde(tag = "code")]
pub enum NodeClientError {
    InternalError,
    BadRequest,
    BadResourcePath,
    NotFound,
    EntityExists,
    NoSuchSession,
    BadAuth,
    InsufficientStorage { message: String },
    ProtocolError { message: String },
    NotEmpty,
    RangeUnsatisfiable,
}

impl std::error::Error for NodeClientError {}

impl error::ResponseError for NodeClientError {
    fn status_code(&self) -> StatusCode {
        match *self {
            NodeClientError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            NodeClientError::BadRequest => StatusCode::BAD_REQUEST,
            NodeClientError::BadResourcePath => StatusCode::BAD_REQUEST,
            NodeClientError::BadAuth => StatusCode::UNAUTHORIZED,
            NodeClientError::InsufficientStorage { .. } => StatusCode::IM_A_TEAPOT,
            NodeClientError::NotFound => StatusCode::NOT_FOUND,
            NodeClientError::NoSuchSession => StatusCode::NOT_FOUND,
            NodeClientError::EntityExists => StatusCode::BAD_REQUEST,
            NodeClientError::NotEmpty => StatusCode::BAD_REQUEST,
            NodeClientError::RangeUnsatisfiable => StatusCode::RANGE_NOT_SATISFIABLE,
            NodeClientError::ProtocolError { .. } => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::json())
            .json(self)
    }
}

impl From<MeowithDataError> for NodeClientError {
    fn from(value: MeowithDataError) -> Self {
        match value {
            MeowithDataError::NotFound => NodeClientError::NotFound,
            MeowithDataError::LockingError => NodeClientError::BadRequest,
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
            MDSFTPError::ReserveError(_) => NodeClientError::InsufficientStorage {
                message: "Failed reservation".to_string(),
            },
            _ => {
                error!("MDSFTP ERROR: {:?}", value);
                NodeClientError::InternalError
            }
        }
    }
}

impl From<Error> for NodeClientError {
    fn from(value: Error) -> Self {
        error!("JWT ERROR: {:?}", value);
        NodeClientError::InternalError
    }
}

impl From<BcryptError> for NodeClientError {
    fn from(value: BcryptError) -> Self {
        error!("BCRYPT ERROR: {:?}", value);
        NodeClientError::InternalError
    }
}

impl From<JoinError> for NodeClientError {
    fn from(value: JoinError) -> Self {
        error!("JOIN ERROR: {:?}", value);
        NodeClientError::InternalError
    }
}

impl From<std::io::Error> for NodeClientError {
    fn from(value: std::io::Error) -> Self {
        error!("STD::IO::ERROR: {:?}", value);
        NodeClientError::InternalError
    }
}

impl From<PayloadError> for NodeClientError {
    fn from(value: PayloadError) -> Self {
        error!("ACTIX_HTTP::ERROR::PAYLOAD_ERROR: {:?}", value);
        NodeClientError::BadRequest
    }
}
