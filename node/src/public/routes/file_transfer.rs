use std::sync::Arc;

use actix_web::http::header::{ContentDisposition, CONTENT_LENGTH};
use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use protocol::mdsftp::handler::{AbstractReadStream, AbstractWriteStream};

use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::public::service::file_access_service::{
    handle_download, handle_upload_durable, handle_upload_oneshot, start_upload_session,
};
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

#[post("/upload/oneshot/{app_id}/{bucket_id}/{path}")]
pub async fn upload_oneshot(
    path: web::Path<(Uuid, Uuid, String)>,
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
        handle_upload_oneshot(
            path.0,
            path.1,
            path.2.clone(),
            content_size,
            app_state,
            accessor,
            abstract_reader,
        )
        .await
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

#[post("/upload/durable/{app_id}/{bucket_id}")]
pub async fn start_upload_durable(
    path: web::Path<(Uuid, Uuid)>,
    accessor: BucketAccessor,
    req: web::Json<UploadSessionRequest>,
    data: web::Data<AppState>,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    start_upload_session(path.0, path.1, accessor, req.0, data).await
}

#[put("/upload/put/{session_id}")]
pub async fn upload_durable(
    path: web::Path<Uuid>,
    accessor: BucketAccessor,
    mut payload: web::Payload,
    data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    let (mut sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);

    let abstract_reader: AbstractReadStream =
        Arc::new(Mutex::new(Box::pin(BufReader::new(receiver))));
    let channel_handle =
        tokio::spawn(
            async move { handle_upload_durable(*path, accessor, abstract_reader, data).await },
        );

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
    path: web::Path<(Uuid, Uuid, String)>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    let (sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);

    let abstract_writer: AbstractWriteStream =
        Arc::new(Mutex::new(Box::pin(BufWriter::new(sender))));
    let info = handle_download(
        path.0,
        path.1,
        path.2.clone(),
        accessor,
        abstract_writer,
        app_data,
    )
    .await?;

    let response_stream = ReaderStream::new(receiver);

    Ok(HttpResponse::Ok()
        .content_type(info.mime)
        .insert_header(ContentDisposition::attachment(info.attachment_name))
        .insert_header((CONTENT_LENGTH, info.size.to_string()))
        .streaming(response_stream))
}
