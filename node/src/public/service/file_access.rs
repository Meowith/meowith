use std::path::Path;
use std::sync::Arc;

use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::web::Data;
use lazy_static::lazy_static;
use mime_guess::mime;
use tokio::io;
use uuid::Uuid;

use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use commons::permission::PermissionList;
use data::access::file_access::get_file;
use data::model::file_model::FileChunk;
use data::model::permission_model::UserPermission;
use protocol::mdsftp::channel::MDSFTPChannel;
use protocol::mdsftp::channel_handle::ChannelAwaitHandle;
use protocol::mdsftp::data::ReserveFlags;
use protocol::mdsftp::error::{MDSFTPError, MDSFTPResult};
use protocol::mdsftp::handler::{AbstractReadStream, AbstractWriteStream};
use protocol::mdsftp::pool::MDSFTPPool;

use crate::file_transfer::channel_handler::MeowithMDSFTPChannelPacketHandler;
use crate::io::fragment_ledger::FragmentLedger;
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::public::routes::file_transfer::{UploadSessionRequest, UploadSessionStartResponse};
use crate::public::service::durable_transfer_session_manager::DurableTransferSessionManager;
use crate::AppState;

// TODO more concise error handling, perhaps a debug!
// call in the from method?
// Main idea being handling notfound vs internal errors proper.

#[allow(unused)]
lazy_static! {
    static ref UPLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Write]).into();
    static ref UPLOAD_OVERWRITE_ALLOWANCE: u64 =
        PermissionList(vec![UserPermission::Write, UserPermission::Overwrite]).into();
    static ref DOWNLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Read]).into();
}

#[allow(unused)]
pub enum ReservationMode {
    PreferSelfThenMostFree,
    PreferMostFree,
}

pub struct DlInfo {
    pub size: u64,
    pub attachment_name: String,
    pub mime: ContentType,
}

pub async fn handle_upload_oneshot(
    app_id: Uuid,
    bucket: Uuid,
    path: String,
    size: u64,
    app_state: Data<AppState>,
    accessor: BucketAccessor,
    reader: AbstractReadStream,
) -> NodeClientResponse<()> {
    // quit early if the user cannot upload at all.
    accessor.has_permission(&bucket, &app_id, *UPLOAD_ALLOWANCE)?;
    let path = split_path(path);

    // check if the file will be overwritten and if the user can do that.
    let file = get_file(bucket, path.0, path.1, &app_state.session).await;

    if let Ok(_) = file {
        accessor.has_permission(&bucket, &app_id, *UPLOAD_OVERWRITE_ALLOWANCE)?;
        // TODO overwrites, config
    }

    let reservation = reserve_chunks(
        size,
        ReserveFlags {
            auto_start: true,
            durable: false,
            temp: false,
        },
        ReservationMode::PreferSelfThenMostFree,
        &app_state.fragment_ledger,
        &app_state.node_storage_map,
        &app_state.req_ctx,
        app_state.mdsftp_server.pool(),
    )
    .await?;

    let mut chunks: Vec<FileChunk> = vec![];
    for (i, space) in (0_i8..).zip(reservation.fragments.into_iter()) {
        chunks.push(FileChunk {
            server_id: space.1,
            chunk_id: space.2,
            chunk_size: space.3 as i64,
            chunk_order: i,
        });

        inbound_transfer(
            reader.clone(),
            space.1,
            space.2,
            space.0,
            space.4,
            space.3,
            &app_state,
        )
        .await?;
    }

    Ok(())
}

pub async fn start_upload_session(
    app_id: Uuid,
    bucket: Uuid,
    accessor: BucketAccessor,
    req: UploadSessionRequest,
    session_manager: &DurableTransferSessionManager,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    accessor
        .has_permission(&bucket, &app_id, *UPLOAD_ALLOWANCE)
        .map_err(|_| NodeClientError::BadRequest)?;
    // TODO validation
    let session_id = session_manager
        .start_session(app_id, bucket, req.path, req.size)
        .await?;
    Ok(web::Json(UploadSessionStartResponse {
        code: session_id.to_string(),
        validity: 0,
    }))
}

pub async fn handle_upload_durable(
    _session_id: Uuid,
    _accessor: BucketAccessor,
    _reader: AbstractReadStream,
) -> NodeClientResponse<ChannelAwaitHandle> {
    todo!()
}

pub async fn handle_download(
    app_id: Uuid,
    bucket: Uuid,
    path: String,
    accessor: BucketAccessor,
    writer: AbstractWriteStream,
    app_state: Data<AppState>,
) -> NodeClientResponse<DlInfo> {
    accessor.has_permission(&bucket, &app_id, *DOWNLOAD_ALLOWANCE)?;
    let path = split_path(path);
    let attachment_name = path.1.clone();
    let file = get_file(bucket, path.0, path.1, &app_state.session)
        .await
        .map_err(|_| NodeClientError::NotFound)?;

    let mut chunk_ids: Vec<&FileChunk> = file.chunk_ids.iter().collect();
    chunk_ids.sort_by_key(|chunk| chunk.chunk_order);

    for chunk in chunk_ids {
        outbound_transfer(writer.clone(), chunk.server_id, chunk.chunk_id, &app_state).await?
    }

    Ok(DlInfo {
        size: file.size as u64,
        mime: ContentType(
            mime_guess::from_path(&attachment_name).first_or(mime::APPLICATION_OCTET_STREAM),
        ),
        attachment_name,
    })
}

async fn inbound_transfer(
    reader: AbstractReadStream,
    node_id: Uuid,
    chunk_id: Uuid,
    channel: Option<MDSFTPChannel>,
    chunk_buffer: u16,
    size: u64,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    if node_id == state.req_ctx.id {
        let writer = state
            .fragment_ledger
            .fragment_write_stream(&chunk_id)
            .await
            .map_err(|_| NodeClientError::InternalError)?;
        let mut writer = writer.lock().await;
        let mut reader = reader.lock().await;
        io::copy(&mut *reader, &mut *writer)
            .await
            .map_err(|_| NodeClientError::InternalError)?;
        Ok(())
    } else {
        let eff_channel: MDSFTPChannel;
        let pool = state.mdsftp_server.pool();
        match channel {
            None => eff_channel = todo!(),
            Some(c) => eff_channel = c,
        }

        let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
            state.fragment_ledger.clone(),
            pool.cfg.buffer_size,
            pool.cfg.fragment_size,
        ));
        let handle = eff_channel
            .send_content(reader, size, chunk_buffer, handler)
            .await
            .map_err(|_| NodeClientError::InternalError)?;

        handle
            .await
            .map_or(Ok(()), |e| e.map_err(|_| NodeClientError::InternalError))?;
        Ok(())
    }
}

async fn outbound_transfer(
    writer: AbstractWriteStream,
    node_id: Uuid,
    chunk_id: Uuid,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    if node_id == state.req_ctx.id {
        // send local chunk, no need for net io
        let reader = state
            .fragment_ledger
            .fragment_read_stream(&chunk_id)
            .await
            .map_err(|_| NodeClientError::NotFound)?;
        let mut writer = writer.lock().await;
        let mut reader = reader.lock().await;
        io::copy(&mut *reader, &mut *writer)
            .await
            .map_err(|_| NodeClientError::InternalError)?;
        Ok(())
    } else {
        let pool = state.mdsftp_server.pool();
        // send remote chunk
        let channel = pool
            .channel(&node_id)
            .await
            .map_err(|_| NodeClientError::InternalError)?;
        let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
            state.fragment_ledger.clone(),
            pool.cfg.buffer_size,
            pool.cfg.fragment_size,
        ));

        let handle = channel
            .retrieve_content(writer, handler, false) // there might be more chunks to send!
            .await
            .map_err(|_| NodeClientError::InternalError)?;

        channel
            .retrieve_req(chunk_id, 16)
            .await
            .map_err(|_| NodeClientError::InternalError)?;

        handle
            .await
            .map_or(Ok(()), |e| e.map_err(|_| NodeClientError::InternalError))
    }
}

#[allow(unused)]
struct ReserveInfo {
    // Channel, none if local, node_id, chunk_id, size, chunk_buffer
    fragments: Vec<(Option<MDSFTPChannel>, Uuid, Uuid, u64, u16)>,
}

#[allow(unused)]
async fn reserve_chunks(
    size: u64,
    flags: ReserveFlags,
    mode: ReservationMode,
    ledger: &FragmentLedger,
    node_map: &NodeStorageMap,
    req_ctx: &Arc<MicroserviceRequestContext>,
    pool: MDSFTPPool,
) -> NodeClientResponse<ReserveInfo> {
    let mut target_list: Vec<(Uuid, u64)> = vec![];
    let self_free = ledger.get_available_space();
    let rem: u64;

    // Figure out targets
    match mode {
        ReservationMode::PreferSelfThenMostFree => {
            if self_free >= size {
                target_list.push((req_ctx.id, size));
                rem = 0;
            } else {
                rem = push_most_used(node_map, &mut target_list, size).await;
            }
        }
        ReservationMode::PreferMostFree => {
            rem = push_most_used(node_map, &mut target_list, size).await;
        }
    }

    if rem > 0 {
        return Err(NodeClientError::InsufficientStorage);
    }

    // Try reserve
    let mut fragments: Vec<(Option<MDSFTPChannel>, Uuid, Uuid, u64, u16)> = vec![];
    let res: MDSFTPResult<()> = async {
        for frag in target_list {
            if frag.0 == req_ctx.id {
                let uuid = ledger
                    .try_reserve(frag.1, flags.durable)
                    .await
                    .map_err(|_| MDSFTPError::ReservationError)?;
                fragments.push((None, frag.0, uuid, frag.1, 0));
            } else {
                let channel = pool.channel(&frag.0).await?;
                let res = channel.try_reserve(frag.1, flags).await?;
                fragments.push((
                    Some(channel),
                    frag.0,
                    res.chunk_id,
                    frag.1,
                    res.chunk_buffer,
                ));
            }
        }
        Ok(())
    }
    .await;

    match res {
        Ok(_) => Ok(ReserveInfo { fragments }),
        Err(_) => {
            // If any reservation fails, release the ones currently acquired
            for frag in fragments {
                if let Some(channel) = frag.0 {
                    let _ = channel.cancel_reserve(frag.2).await;
                } else {
                    let _ = ledger.cancel_reservation(&frag.2).await;
                }
            }

            Err(NodeClientError::InsufficientStorage)
        }
    }
}

async fn push_most_used(
    node_map: &NodeStorageMap,
    target_list: &mut Vec<(Uuid, u64)>,
    mut size: u64,
) -> u64 {
    let node_map_read = node_map.read().await;

    let mut nodes: Vec<(&Uuid, &u64)> = node_map_read.iter().collect();
    nodes.sort_by(|a, b| b.1.cmp(a.1));

    for (uuid, &node_size) in nodes {
        if size == 0 {
            break;
        }

        if size >= node_size {
            size -= node_size;
            target_list.push((*uuid, node_size));
        } else {
            target_list.push((*uuid, size));
            size = 0;
        }
    }

    size
}

/// Split the given file path into a format friendly to the database
///
/// ```
/// // path                (dir, name)
/// "/a/path/to/a.txt"  => ("a/path/to", "a.txt")
/// "a/path/to/a.txt"   => ("a/path/to", "a.txt")
/// "a\\path\\to/a.txt" => ("a/path/to", "a.txt")
/// "a.txt"             => ("", "a.txt")
/// ```
fn split_path(file_path: String) -> (String, String) {
    let normalized_path = file_path.replace('\\', "/");

    let path = Path::new(&normalized_path);
    let parent = path.parent().unwrap_or(Path::new("")).to_path_buf();
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let mut parent_str = parent.to_string_lossy().into_owned();

    // Strip leading and trailing slashes
    parent_str = parent_str.trim_matches('/').to_string();

    (parent_str, file_name)
}

#[cfg(test)]
mod tests {
    use crate::public::service::file_access::split_path;

    #[test]
    fn test_split_path() {
        let cases = vec![
            ("/a/path/to/a.txt", "a/path/to", "a.txt"),
            ("a/path/to/a.txt", "a/path/to", "a.txt"),
            ("a\\path\\to/a.txt", "a/path/to", "a.txt"),
            ("a.txt", "", "a.txt"),
        ];

        for case in cases {
            assert_eq!(
                split_path(case.0.to_owned()),
                (case.1.to_string(), case.2.to_string())
            );
        }
    }
}
