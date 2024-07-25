use std::sync::Arc;

use crate::file_transfer::channel_handler::{AbstractReadStream, AbstractWriteStream};
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::public::service::file_access::{
    handle_download, handle_upload_durable, handle_upload_oneshot, start_upload_session,
};

use actix_web::http::header::{CONTENT_LENGTH, ContentDisposition};
use actix_web::{get, post, put, web, HttpResponse};
use futures_util::{StreamExt,};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;
use uuid::Uuid;
use crate::AppState;

const USER_TRANSFER_BUFFER: usize = 1024;

#[derive(Serialize)]
#[allow(unused)]
pub struct UploadSessionStartResponse {
    /// To be used in the X-UploadCode header
    pub code: String,
    /// Seconds till the unfinished chunk is dropped when the upload is not reinitialized
    pub validity: u32,
}

#[derive(Deserialize)]
#[allow(unused)]
pub struct UploadSessionRequest {
    /// Entry size in bytes
    pub size: u64,
    /// Entry full path
    pub path: String,
}

#[post("/upload/oneshot/{app_id}/{bucket_id}")]
pub async fn upload_oneshot(
    path: web::Path<(Uuid, String)>,
    accessor: BucketAccessor,
    mut payload: web::Payload,
) -> NodeClientResponse<HttpResponse> {
    let (mut sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);

    let abstract_reader: AbstractReadStream =
        Arc::new(Mutex::new(BufReader::new(Box::pin(receiver))));
    let channel_handle =
        handle_upload_oneshot(path.0, path.1.clone(), accessor, abstract_reader).await?;

    while let Some(item) = payload.next().await {
        let item = item.map_err(|_| NodeClientError::BadRequest)?;
        sender
            .write_all(&item)
            .await
            .map_err(|_| NodeClientError::InternalError)?;
    }

    match channel_handle.await {
        None => {}
        Some(err) => err.map_err(|_| NodeClientError::InternalError)?,
    }

    Ok(HttpResponse::Ok().finish())
}

#[post("/upload/durable/{app_id}/{bucket_id}")]
pub async fn start_upload_durable(
    path: web::Path<(Uuid, String)>,
    accessor: BucketAccessor,
    req: web::Json<UploadSessionRequest>,
    data: web::Data<AppState>
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    start_upload_session(path.0, path.1.clone(), accessor, req.0, &data.upload_manager).await
}

#[put("/upload/put/{session_id}")]
pub async fn upload_durable(
    path: web::Path<Uuid>,
    accessor: BucketAccessor,
    mut payload: web::Payload,
) -> NodeClientResponse<HttpResponse> {
    let (mut sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);

    let abstract_reader: AbstractReadStream =
        Arc::new(Mutex::new(BufReader::new(Box::pin(receiver))));
    let channel_handle = handle_upload_durable(*path, accessor, abstract_reader).await?;

    while let Some(item) = payload.next().await {
        let item = item.map_err(|_| NodeClientError::BadRequest)?;
        sender
            .write_all(&item)
            .await
            .map_err(|_| NodeClientError::InternalError)?;
    }

    match channel_handle.await {
        None => {}
        Some(err) => err.map_err(|_| NodeClientError::InternalError)?,
    }

    Ok(HttpResponse::Ok().finish())
}

#[get("/download/{app_id}/{bucket_id}/{path}")]
pub async fn download(
    path: web::Path<(Uuid, String, String)>,
    accessor: BucketAccessor,
) -> NodeClientResponse<HttpResponse> {
    let (sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);

    let abstract_writer: AbstractWriteStream =
        Arc::new(Mutex::new(BufWriter::new(Box::pin(sender))));
    let info = handle_download(
        path.0,
        path.1.clone(),
        path.2.clone(),
        accessor,
        abstract_writer,
    )
    .await?;

    let response_stream = ReaderStream::new(receiver);

    Ok(HttpResponse::Ok()
        .content_type(info.mime)
        .insert_header(ContentDisposition::attachment(info.attachment_name))
        .insert_header((CONTENT_LENGTH, info.size.to_string()))
        .streaming(response_stream))
}
