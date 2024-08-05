use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::web::Data;
use chrono::Utc;
use futures_util::future::try_join_all;
use lazy_static::lazy_static;
use log::debug;
use mime_guess::mime;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::{io, time};
use uuid::Uuid;

use commons::context::microservice_request_context::NodeStorageMap;
use commons::permission::PermissionList;
use data::access::file_access::{
    delete_file, get_bucket, get_file, insert_file, update_upload_session_last_access,
};
use data::model::file_model::{Bucket, BucketUploadSession, File, FileChunk};
use data::model::permission_model::UserPermission;
use logging::log_err;
use protocol::mdsftp::channel::MDSFTPChannel;
use protocol::mdsftp::data::{CommitFlags, PutFlags, ReserveFlags};
use protocol::mdsftp::error::{MDSFTPError, MDSFTPResult};
use protocol::mdsftp::handler::{AbstractReadStream, AbstractWriteStream};

use crate::file_transfer::channel_handler::MeowithMDSFTPChannelPacketHandler;
use crate::io::error::MeowithIoError;
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::public::routes::file_transfer::{UploadSessionRequest, UploadSessionStartResponse};
use crate::AppState;

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
    bucket_id: Uuid,
    path: String,
    size: u64,
    app_state: Data<AppState>,
    accessor: BucketAccessor,
    reader: AbstractReadStream,
) -> NodeClientResponse<()> {
    // quit early if the user cannot upload at all.
    accessor.has_permission(&bucket_id, &app_id, *UPLOAD_ALLOWANCE)?;
    let split_path = split_path(path.clone());

    let bucket = get_bucket(app_id, bucket_id, &app_state.session).await?;

    // check if the file will be overwritten and if the user can do that.
    let file = get_file(
        bucket_id,
        split_path.0.clone(),
        split_path.1.clone(),
        &app_state.session,
    )
    .await;
    let mut old_file: Option<File> = None;
    let overwrite = if file.is_ok() {
        accessor.has_permission(&bucket_id, &app_id, *UPLOAD_OVERWRITE_ALLOWANCE)?;
        old_file = Some(file.unwrap());
        if !bucket.atomic_upload {
            do_delete_file(old_file.as_ref().unwrap(), &bucket, &app_state).await?;
        }
        true
    } else {
        false
    };

    let reserved = app_state
        .upload_manager
        .get_reserved_space(app_id, bucket_id)
        .await?;
    if bucket.space_taken.0 + size as i64 + reserved > bucket.quota {
        return Err(NodeClientError::InsufficientStorage);
    }

    let reservation = reserve_chunks(
        size,
        ReserveFlags {
            auto_start: true,
            durable: false,
            temp: false,
            overwrite,
        },
        ReservationMode::PreferSelfThenMostFree,
        &app_state,
    )
    .await?;

    let session_id = app_state
        .upload_manager
        .start_session(BucketUploadSession {
            app_id,
            bucket: bucket_id,
            id: Uuid::new_v4(),
            path,
            size: size as i64,
            completed: 0,
            durable: false,
            fragments: reserve_info_to_file_chunks(&reservation),
            last_access: Utc::now(),
        })
        .await?;

    let data = app_state.clone();
    let chunks: Arc<Mutex<HashSet<FileChunk>>> = Default::default();
    let chunks_clone = chunks.clone();
    let session_clone = app_state.clone();

    let notifier = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut futures = vec![];
            for chunk in &*chunks_clone.lock().await {
                futures.push(commit_chunk(
                    CommitFlags::keep_alive(),
                    chunk.server_id,
                    chunk.chunk_id,
                    &data,
                ));
            }
            let _ = try_join_all(futures).await;
            let _ = update_upload_session_last_access(
                app_id,
                bucket_id,
                session_id,
                Utc::now(),
                &session_clone.session,
            )
            .await;
        }
    });

    let transfer_result: NodeClientResponse<()> = async {
        for (i, space) in (0_i8..).zip(reservation.fragments.into_iter()) {
            chunks.lock().await.insert(FileChunk {
                server_id: space.node_id,
                chunk_id: space.chunk_id,
                chunk_size: space.size as i64,
                chunk_order: i,
            });

            inbound_transfer(
                reader.clone(),
                space.node_id,
                space.chunk_id,
                space.channel,
                ChunkInfo {
                    chunk_buffer: space.chunk_buffer,
                    size: space.size,
                    append: false, // always the case for non-durable uploads.
                },
                &app_state,
            )
            .await?;
        }
        Ok(())
    }
    .await;

    if transfer_result.is_err() {
        let err = transfer_result.unwrap_err();
        debug!("Oneshot upload failure, deleting. {}", &err);

        let mut futures = vec![];
        for chunk in &*chunks.lock().await {
            futures.push(commit_chunk(
                CommitFlags::reject(),
                chunk.server_id,
                chunk.chunk_id,
                &app_state,
            ));
        }
        try_join_all(futures).await?;
        app_state
            .upload_manager
            .end_session(app_id, bucket_id, session_id)
            .await;

        return Err(err);
    }

    notifier.abort();

    let mut futures = vec![];
    for chunk in &*chunks.lock().await {
        futures.push(commit_chunk(
            CommitFlags::r#final(),
            chunk.server_id,
            chunk.server_id,
            &app_state,
        ))
    }
    try_join_all(futures).await?;

    let now = Utc::now();
    let chunks = chunks.lock().await.clone();

    let file = File {
        bucket_id,
        directory: split_path.0,
        name: split_path.1,
        size: size as i64,
        chunk_ids: chunks,
        created: now,
        last_modified: now,
    };

    insert_file(&file, &bucket, &app_state.session).await?;
    app_state
        .upload_manager
        .end_session(app_id, bucket_id, session_id)
        .await;

    if bucket.atomic_upload && old_file.is_some() {
        do_delete_file(old_file.as_ref().unwrap(), &bucket, &app_state).await?;
    }

    Ok(())
}

pub async fn start_upload_session(
    app_id: Uuid,
    bucket_id: Uuid,
    accessor: BucketAccessor,
    req: UploadSessionRequest,
    app_state: Data<AppState>,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    accessor
        .has_permission(&bucket_id, &app_id, *UPLOAD_ALLOWANCE)
        .map_err(|_| NodeClientError::BadRequest)?;

    let path = split_path(req.path.clone());

    let bucket = get_bucket(app_id, bucket_id, &app_state.session).await?;

    // check if the file will be overwritten and if the user can do that.
    let file = get_file(bucket_id, path.0, path.1, &app_state.session).await;
    let overwrite = if file.is_ok() {
        accessor.has_permission(&bucket_id, &app_id, *UPLOAD_OVERWRITE_ALLOWANCE)?;
        let file = file.unwrap();
        if !bucket.atomic_upload {
            do_delete_file(&file, &bucket, &app_state).await?;
        }
        true
    } else {
        false
    };

    let reserved = app_state
        .upload_manager
        .get_reserved_space(app_id, bucket_id)
        .await?;
    if bucket.space_taken.0 + req.size as i64 + reserved > bucket.quota {
        return Err(NodeClientError::InsufficientStorage);
    }

    let reservation = reserve_chunks(
        req.size,
        ReserveFlags {
            auto_start: false,
            durable: true,
            temp: true,
            overwrite,
        },
        ReservationMode::PreferSelfThenMostFree,
        &app_state,
    )
    .await?;

    let session_id = app_state
        .upload_manager
        .start_session(BucketUploadSession {
            app_id,
            bucket: bucket_id,
            id: Uuid::new_v4(),
            path: req.path,
            size: req.size as i64,
            completed: 0,
            durable: true,
            fragments: reserve_info_to_file_chunks(&reservation),
            last_access: Utc::now(),
        })
        .await?;
    Ok(web::Json(UploadSessionStartResponse {
        code: session_id.to_string(),
        validity: 0,
    }))
}

pub async fn handle_upload_durable(
    session_id: Uuid,
    app_id: Uuid,
    bucket_id: Uuid,
    _accessor: BucketAccessor,
    _reader: AbstractReadStream,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    let _session = app_state
        .upload_manager
        .get_session(app_id, bucket_id, session_id)
        .await?;

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
    let file = get_file(bucket, path.0, path.1, &app_state.session).await?;

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

async fn commit_chunk(
    flags: CommitFlags,
    node_id: Uuid,
    chunk_id: Uuid,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    if node_id == state.req_ctx.id {
        if flags.r#final {
            let _ = state.fragment_ledger.commit_chunk(&chunk_id).await;
        } else if flags.keep_alive {
            let _ = state.fragment_ledger.commit_alive(&chunk_id).await;
        } else if flags.reject {
            let _ = state.fragment_ledger.delete_chunk(&chunk_id).await;
        }
        Ok(())
    } else {
        let pool = state.mdsftp_server.pool();
        let channel = pool.channel(&node_id).await?;
        channel
            .commit(chunk_id, flags)
            .await
            .map_err(NodeClientError::from)
    }
}

#[allow(unused)]
async fn delete_chunk(
    node_id: Uuid,
    chunk_id: Uuid,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    if node_id == state.req_ctx.id {
        state
            .fragment_ledger
            .delete_chunk(&chunk_id)
            .await
            .map_err(|_| NodeClientError::InternalError)
    } else {
        let pool = state.mdsftp_server.pool();
        let channel = pool.channel(&node_id).await?;
        channel
            .delete_chunk(chunk_id)
            .await
            .map_err(NodeClientError::from)
    }
}

pub struct ChunkInfo {
    pub chunk_buffer: u16,
    pub size: u64,
    pub append: bool,
}

async fn do_delete_file(
    file: &File,
    bucket: &Bucket,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    for chunk in &file.chunk_ids {
        if chunk.server_id == state.req_ctx.id {
            log_err(
                "file delete error",
                state.fragment_ledger.delete_chunk(&chunk.chunk_id).await,
            );
        } else if let Ok(channel) = state.mdsftp_server.pool().channel(&chunk.server_id).await {
            log_err(
                "file delete error",
                channel.delete_chunk(chunk.chunk_id).await,
            );
        }
    }

    delete_file(file, bucket, &state.session).await?;

    Ok(())
}

async fn inbound_transfer(
    reader: AbstractReadStream,
    node_id: Uuid,
    chunk_id: Uuid,
    channel: Option<MDSFTPChannel>,
    chunk: ChunkInfo,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    if node_id == state.req_ctx.id {
        let writer = if chunk.append {
            state.fragment_ledger.fragment_write_stream(&chunk_id).await
        } else {
            state
                .fragment_ledger
                .fragment_append_stream(&chunk_id)
                .await
        }
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
            None => {
                let channel = pool.channel(&node_id).await?;
                channel
                    .request_put(
                        PutFlags {
                            append: chunk.append,
                        },
                        chunk_id,
                        chunk.size,
                    )
                    .await?;
                eff_channel = channel;
            }
            Some(c) => eff_channel = c,
        }

        let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
            state.fragment_ledger.clone(),
            pool.cfg.buffer_size,
            pool.cfg.fragment_size,
        ));
        let handle = eff_channel
            .send_content(reader, chunk.size, chunk.chunk_buffer, handler)
            .await?;

        handle.await.map_or(Ok(()), |e| e)?;
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
        let channel = pool.channel(&node_id).await?;
        let handler = Box::new(MeowithMDSFTPChannelPacketHandler::new(
            state.fragment_ledger.clone(),
            pool.cfg.buffer_size,
            pool.cfg.fragment_size,
        ));

        let handle = channel
            .retrieve_content(writer, handler, false) // there might be more chunks to send!
            .await?;

        channel.retrieve_req(chunk_id, 16).await?;

        handle
            .await
            .map_or(Ok(()), |e| e.map_err(NodeClientError::from))
    }
}

pub struct ReservedFragment {
    pub channel: Option<MDSFTPChannel>,
    pub node_id: Uuid,
    pub chunk_id: Uuid,
    pub size: u64,
    pub chunk_buffer: u16,
}

pub struct ReserveInfo {
    // Channel, none if local, node_id, chunk_id, size, chunk_buffer
    pub fragments: Vec<ReservedFragment>,
}

pub fn reserve_info_to_file_chunks(reserve_info: &ReserveInfo) -> HashSet<FileChunk> {
    (0_i8..)
        .zip(reserve_info.fragments.iter())
        .map(|chunk| FileChunk {
            server_id: chunk.1.node_id,
            chunk_id: chunk.1.chunk_id,
            chunk_size: chunk.1.size as i64,
            chunk_order: chunk.0,
        })
        .collect()
}

#[allow(unused)]
async fn reserve_chunks(
    size: u64,
    flags: ReserveFlags,
    mode: ReservationMode,
    state: &Data<AppState>,
) -> NodeClientResponse<ReserveInfo> {
    let mut target_list: Vec<(Uuid, u64)> = vec![];
    let self_free = state.fragment_ledger.get_available_space();
    let rem: u64;

    // Figure out targets
    match mode {
        ReservationMode::PreferSelfThenMostFree => {
            if self_free >= size {
                target_list.push((state.req_ctx.id, size));
                rem = 0;
            } else {
                rem = push_most_used(&state.node_storage_map, &mut target_list, size).await;
            }
        }
        ReservationMode::PreferMostFree => {
            rem = push_most_used(&state.node_storage_map, &mut target_list, size).await;
        }
    }

    if rem > 0 {
        return Err(NodeClientError::InsufficientStorage);
    }

    // Try reserve
    let mut fragments: Vec<ReservedFragment> = vec![];
    let res: MDSFTPResult<()> = async {
        let pool = state.mdsftp_server.pool();
        for frag in target_list {
            if frag.0 == state.req_ctx.id {
                let uuid = match state
                    .fragment_ledger
                    .try_reserve(frag.1, flags.durable)
                    .await
                {
                    Ok(id) => Ok(id),
                    Err(MeowithIoError::InsufficientDiskSpace) => {
                        Err(MDSFTPError::ReservationError)
                    }
                    Err(_) => Err(MDSFTPError::Internal),
                }?;

                fragments.push(ReservedFragment {
                    channel: None,
                    node_id: frag.0,
                    chunk_id: uuid,
                    size: frag.1,
                    chunk_buffer: 0,
                });
            } else {
                let channel = pool.channel(&frag.0).await?;
                let res = channel.try_reserve(frag.1, flags).await;
                match res {
                    Ok(res) => {
                        fragments.push(ReservedFragment {
                            channel: Some(channel),
                            node_id: frag.0,
                            chunk_id: res.chunk_id,
                            size: frag.1,
                            chunk_buffer: res.chunk_buffer,
                        });
                        Ok(())
                    }
                    Err(MDSFTPError::ReserveError(free_space)) => {
                        let mut write = state.node_storage_map.write().await;
                        write.insert(frag.0, free_space);
                        Err(MDSFTPError::ReservationError)
                    }
                    Err(e) => Err(e),
                }?;
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
                if let Some(channel) = frag.channel {
                    let _ = channel.cancel_reserve(frag.chunk_id).await;
                } else {
                    let _ = state
                        .fragment_ledger
                        .cancel_reservation(&frag.chunk_id)
                        .await;
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
    use crate::public::service::file_access_service::split_path;

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
