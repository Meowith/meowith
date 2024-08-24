use std::sync::Arc;

use actix_web::http::header::{ContentDisposition, CONTENT_LENGTH};
use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use log::error;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use protocol::mdsftp::handler::{AbstractReadStream, AbstractWriteStream};

use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::routes::EntryPath;
use crate::public::service::file_access_service::{
    handle_download, handle_upload_durable, handle_upload_oneshot, resume_upload_session,
    start_upload_session,
};
use crate::AppState;
use commons::error::std_response::{NodeClientError, NodeClientResponse};

const USER_TRANSFER_BUFFER: usize = 8 * 1024;

#[derive(Serialize)]

pub struct UploadSessionStartResponse {
    /// To be used in the path
    pub code: String,
    /// Seconds till the unfinished chunk is dropped when the upload is not reinitialized
    pub validity: u32,
    /// The amount already uploaded to meowith.
    /// The client should resume uploading from there.
    pub uploaded: u64,
}

#[derive(Deserialize)]
pub struct UploadSessionRequest {
    /// Entry size in bytes
    pub size: u64,
}

#[derive(Serialize)]
pub struct UploadSessionResumeResponse {
    /// The number of bytes already uploaded to the meowith store.
    pub uploaded_size: u64,
}

#[derive(Deserialize)]
pub struct UploadSessionResumeRequest {
    pub session_id: Uuid,
}

#[post("/upload/oneshot/{app_id}/{bucket_id}/{path}")]
pub async fn upload_oneshot(
    path: web::Path<EntryPath>,
    accessor: BucketAccessor,
    req: HttpRequest,
    app_state: web::Data<AppState>,
    mut payload: web::Payload,
) -> NodeClientResponse<HttpResponse> {
    let content_size: u64 = req
        .headers()
        .get(CONTENT_LENGTH)
        .ok_or(NodeClientError::BadRequest)?
        .to_str()
        .map_err(|_| NodeClientError::BadRequest)?
        .parse()
        .map_err(|_| NodeClientError::BadRequest)?;

    let (mut sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);
    let abstract_reader: AbstractReadStream =
        Arc::new(Mutex::new(Box::pin(BufReader::new(receiver))));
    let channel_handle = tokio::spawn(async move {
        let err = handle_upload_oneshot(
            path.into_inner(),
            content_size,
            app_state,
            accessor,
            abstract_reader,
        )
        .await;
        if let Err(err) = err {
            error!("ONESHOT UPLOAD ERR: {err:?}");
            return Err(err);
        }
        Ok(())
    });

    while let Some(item) = payload.next().await {
        let item = item.map_err(|_| NodeClientError::BadRequest)?;
        sender.write_all(&item).await?;
    }

    channel_handle.await??;

    Ok(HttpResponse::Ok().finish())
}

#[post("/upload/durable/{app_id}/{bucket_id}/{path}")]
pub async fn start_upload_durable(
    path: web::Path<EntryPath>,
    accessor: BucketAccessor,
    req: web::Json<UploadSessionRequest>,
    data: web::Data<AppState>,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    start_upload_session(path.into_inner(), accessor, req.0, data).await
}

#[post("/upload/resume/{app_id}/{bucket_id}")]
pub async fn resume_durable_upload(
    path: web::Path<(Uuid, Uuid)>,
    req: web::Json<UploadSessionResumeRequest>,
    data: web::Data<AppState>,
) -> NodeClientResponse<web::Json<UploadSessionResumeResponse>> {
    resume_upload_session(path.0, path.1, req.session_id, data)
        .await
        .map(|size| {
            web::Json(UploadSessionResumeResponse {
                uploaded_size: size as u64,
            })
        })
}

#[put("/upload/put/{app_id}/{bucket_id}/{session_id}")]
pub async fn upload_durable(
    path: web::Path<(Uuid, Uuid, Uuid)>,
    accessor: BucketAccessor,
    mut payload: web::Payload,
    data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    let (mut sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);

    let abstract_reader: AbstractReadStream =
        Arc::new(Mutex::new(Box::pin(BufReader::new(receiver))));
    let channel_handle = tokio::spawn(async move {
        handle_upload_durable(path.2, path.0, path.1, accessor, abstract_reader, data).await
    });

    while let Some(item) = payload.next().await {
        let item = item.map_err(|_| NodeClientError::BadRequest)?;
        sender
            .write_all(&item)
            .await
            .map_err(|_| NodeClientError::InternalError)?;
    }

    channel_handle
        .await
        .map_err(|_| NodeClientError::InternalError)??;

    Ok(HttpResponse::Ok().finish())
}

#[get("/download/{app_id}/{bucket_id}/{path}")]
pub async fn download(
    path: web::Path<EntryPath>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    let (sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);

    let abstract_writer: AbstractWriteStream =
        Arc::new(Mutex::new(Box::pin(BufWriter::new(sender))));
    let (info, _) = handle_download(path.into_inner(), accessor, abstract_writer, app_data).await?;

    let response_stream = ReaderStream::new(receiver);

    Ok(HttpResponse::Ok()
        .content_type(info.mime)
        .insert_header(ContentDisposition::attachment(info.attachment_name))
        .insert_header((CONTENT_LENGTH, info.size.to_string()))
        .streaming(response_stream))
}
