use std::sync::Arc;

use actix_web::http::header::{ContentDisposition, ContentLength, Header, Range, CONTENT_LENGTH};
use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use log::{trace, warn};
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::select;
use tokio::sync::Mutex;
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;
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
use data::dto::entity::{
    UploadSessionRequest, UploadSessionResumeRequest, UploadSessionResumeResponse,
    UploadSessionStartResponse,
};

const USER_TRANSFER_BUFFER: usize = 8 * 1024;

#[post("/upload/oneshot/{app_id}/{bucket_id}/{path:.*}")]
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
            warn!("Oneshot upload error: {err:?}");
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

#[post("/upload/durable/{app_id}/{bucket_id}/{path:.*}")]
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
    let token = CancellationToken::new();
    let cancel_sender = token.clone();

    let channel_handle = tokio::spawn(async move {
        let res =
            handle_upload_durable(path.2, path.0, path.1, accessor, abstract_reader, data).await;
        trace!("Durable upload finished {res:?}");
        cancel_sender.cancel();
        res
    });

    let send_res: NodeClientResponse<()> = async {
        while let Some(item) = select! {
            _ = token.cancelled() => {
                return Ok(());
            },
            data = payload.next() => { data }
        } {
            let item = item.map_err(|_| NodeClientError::BadRequest)?;

            sender
                .write_all(&item)
                .await
                .map_err(|_| NodeClientError::InternalError)?;
        }
        Ok(())
    }
    .await;

    sender.shutdown().await?;
    drop(sender);

    channel_handle
        .await
        .map_err(|_| NodeClientError::InternalError)??;

    send_res.map(|_| HttpResponse::Ok().finish())
}

#[get("/download/{app_id}/{bucket_id}/{path:.*}")]
pub async fn download(
    path: web::Path<EntryPath>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    req: HttpRequest,
) -> NodeClientResponse<HttpResponse> {
    let (sender, receiver) = tokio::io::duplex(USER_TRANSFER_BUFFER);
    let range = Range::parse(&req);
    let mut range_clone = None;
    let mut byte_range = None;

    if let Ok(range) = range {
        match range {
            Range::Bytes(range) => {
                if range.len() != 1 {
                    return Err(NodeClientError::RangeUnsatisfiable);
                }
                byte_range = Some(range[0].clone());
                range_clone.clone_from(&byte_range);
            }
            Range::Unregistered(_, _) => {}
        }
    }

    let abstract_writer: AbstractWriteStream =
        Arc::new(Mutex::new(Box::pin(BufWriter::new(sender))));
    let (info, _) = handle_download(
        path.into_inner(),
        accessor,
        abstract_writer,
        app_data,
        byte_range.clone(),
    )
    .await?;

    let response_stream = ReaderStream::new(receiver);

    let len = byte_range
        .map(|x| {
            x.to_satisfiable_range(info.size)
                .map(|y| y.1 - y.0 + 1)
                .unwrap_or(info.size)
        })
        .unwrap_or(info.size);

    Ok(if range_clone.is_some() {
        HttpResponse::PartialContent()
    } else {
        HttpResponse::Ok()
    }
    .content_type(info.mime)
    .insert_header(ContentLength(len as usize))
    .insert_header(("X-File-Content-Length", len as usize))
    .insert_header(ContentDisposition::attachment(info.attachment_name))
    .streaming(response_stream))
}
