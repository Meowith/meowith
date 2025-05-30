use std::cmp::{max, min};
use std::sync::Arc;
use std::time::Duration;

use actix_web::http::header::{ByteRangeSpec, ContentType};
use actix_web::web;
use actix_web::web::Data;
use chrono::Utc;
use futures_util::future::try_join_all;
use log::{debug, error, trace, warn};
use mime_guess::mime;
use scylla::client::caching_session::CachingSession;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time;
use uuid::Uuid;

use data::access::file_access::{
    get_bucket, get_directory, get_file, get_file_dir, insert_directory, insert_file, ROOT_DIR,
};
use data::model::file_model::{
    Bucket, BucketUploadSession, Directory, File, FileChunk, SessionState,
};
use protocol::mdsftp::data::{ChunkRange, CommitFlags, ReserveFlags};
use protocol::mdsftp::handler::{AbstractReadStream, AbstractWriteStream};

use crate::public::extractors::entry_path::EntryPath;
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::service::chunk_service::{commit_chunk, query_chunk, ChunkInfo};
use crate::public::service::durable_transfer_session_manager::DURABLE_UPLOAD_SESSION_VALIDITY_TIME_SECS;
use crate::public::service::file_action_service::do_delete_file;
use crate::public::service::file_io_service::{inbound_transfer, outbound_transfer};
use crate::public::service::reservation_service::{
    reserve_chunks, reserve_info_to_file_chunks, ReservationMode,
};
use crate::public::service::{DOWNLOAD_ALLOWANCE, UPLOAD_ALLOWANCE, UPLOAD_OVERWRITE_ALLOWANCE};
use crate::AppState;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::dto::entity::{UploadSessionRequest, UploadSessionStartResponse};
use data::pathlib::split_path;

pub struct DlInfo {
    pub size: u64,
    pub attachment_name: String,
    pub mime: ContentType,
}

pub async fn handle_upload_oneshot(
    path: EntryPath,
    size: u64,
    app_state: Data<AppState>,
    accessor: BucketAccessor,
    reader: AbstractReadStream,
) -> NodeClientResponse<()> {
    // quit early if the user cannot upload at all.
    accessor.has_permission(&path.app_id, &path.bucket_id, *UPLOAD_ALLOWANCE)?;
    let split_path = split_path(&path.path());

    let bucket = get_bucket(path.app_id, path.bucket_id, &app_state.session).await?;

    // check if the file will be overwritten and if the user can do that.
    let file = get_file_dir(
        path.bucket_id,
        split_path.0.clone(),
        split_path.1.clone(),
        &app_state.session,
    )
    .await;
    let mut old_file: Option<File> = None;
    let overwrite = if file.is_ok() {
        accessor.has_permission(&path.app_id, &path.bucket_id, *UPLOAD_OVERWRITE_ALLOWANCE)?;
        old_file = Some(file?.0);
        if !bucket.atomic_upload {
            trace!("Overwriting old file {}", path.path());
            do_delete_file(old_file.as_ref().unwrap(), &bucket, &app_state).await?;
            old_file = None;
        }
        true
    } else {
        false
    };

    let reserved = app_state
        .upload_manager
        .get_reserved_space(path.app_id, path.bucket_id)
        .await?;
    if bucket.space_taken + size as i64 + reserved > bucket.quota {
        return Err(NodeClientError::InsufficientStorage {
            message: format!(
                "Insufficient space in bucket. quota={}, size={}, taken={}, reserved={}",
                bucket.quota, size, bucket.space_taken, reserved
            ),
        });
    }

    let file_id = Uuid::new_v4();
    let reservation = reserve_chunks(
        size,
        ReserveFlags {
            auto_start: true,
            durable: false,
            temp: false,
            overwrite,
        },
        bucket.id,
        file_id,
        ReservationMode::PreferSelfThenMostFree,
        &app_state,
    )
    .await?;

    let bucket_upload_session = BucketUploadSession {
        app_id: path.app_id,
        bucket: path.bucket_id,
        file_id,
        id: Uuid::new_v4(),
        path: path.path(),
        size: size as i64,
        durable: false,
        fragments: reserve_info_to_file_chunks(&reservation),
        last_access: Utc::now(),
        state: SessionState::Writing.into(),
    };

    let session_id = app_state
        .upload_manager
        .start_session(&bucket_upload_session)
        .await?;

    let bucket_upload_session = Arc::new(Mutex::new(bucket_upload_session));
    let notifier = create_commit_notifier(bucket_upload_session.clone(), app_state.clone());

    let transfer_result: NodeClientResponse<()> = async {
        for space in reservation.fragments.into_iter() {
            inbound_transfer(
                reader.clone(),
                0,
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

    trace!("Aborting the notifier");
    // We are getting the session BEFORE the .abort call to ensure it is not in the middle of
    // updating the sessions last_access
    let mut session = bucket_upload_session.lock().await;
    notifier.abort();

    if transfer_result.is_err() {
        let err = transfer_result.unwrap_err();
        debug!("Oneshot upload failure, deleting. {}", &err);

        let mut futures = vec![];
        for chunk in &session.fragments {
            futures.push(commit_chunk(
                CommitFlags::reject(),
                chunk.server_id,
                chunk.chunk_id,
                &app_state,
            ));
        }
        app_state
            .upload_manager
            .end_session(path.app_id, path.bucket_id, session_id)
            .await;
        try_join_all(futures).await?;

        return Err(err);
    }

    end_session(
        app_state,
        split_path,
        size as i64,
        &mut session,
        bucket,
        (path.app_id, session_id),
        Some(old_file),
        file_id,
    )
    .await
}

pub async fn start_upload_session(
    e_path: EntryPath,
    accessor: BucketAccessor,
    req: UploadSessionRequest,
    app_state: Data<AppState>,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    accessor
        .has_permission(&e_path.app_id, &e_path.bucket_id, *UPLOAD_ALLOWANCE)
        .map_err(|_| NodeClientError::BadRequest)?;

    let path = split_path(&e_path.path());

    let bucket = get_bucket(e_path.app_id, e_path.bucket_id, &app_state.session).await?;

    // check if the file will be overwritten and if the user can do that.
    let file = get_file_dir(e_path.bucket_id, path.0, path.1, &app_state.session).await;
    let overwrite = if file.is_ok() {
        accessor.has_permission(
            &e_path.app_id,
            &e_path.bucket_id,
            *UPLOAD_OVERWRITE_ALLOWANCE,
        )?;
        let file = file?;
        if !bucket.atomic_upload {
            do_delete_file(&file.0, &bucket, &app_state).await?;
        }
        true
    } else {
        false
    };

    let reserved = app_state
        .upload_manager
        .get_reserved_space(e_path.app_id, e_path.bucket_id)
        .await?;
    if bucket.space_taken + req.size as i64 + reserved > bucket.quota {
        return Err(NodeClientError::InsufficientStorage {
            message: format!(
                "Insufficient space in bucket. quota={}, size={}, taken={}, reserved={}",
                bucket.quota, req.size, bucket.space_taken, reserved
            ),
        });
    }

    let file_id = Uuid::new_v4();
    let reservation = reserve_chunks(
        req.size,
        ReserveFlags {
            auto_start: false,
            durable: true,
            temp: true,
            overwrite,
        },
        bucket.id,
        file_id,
        ReservationMode::PreferSelfThenMostFree,
        &app_state,
    )
    .await?;

    let bucket_upload_session = BucketUploadSession {
        app_id: e_path.app_id,
        bucket: e_path.bucket_id,
        file_id,
        id: Uuid::new_v4(),
        path: e_path.path(),
        size: req.size as i64,
        durable: true,
        fragments: reserve_info_to_file_chunks(&reservation),
        last_access: Utc::now(),
        state: SessionState::AwaitingData.into(),
    };

    trace!("Starting upload session {bucket_upload_session:?}");

    let session_id = app_state
        .upload_manager
        .start_session(&bucket_upload_session)
        .await?;
    Ok(web::Json(UploadSessionStartResponse {
        code: session_id.to_string(),
        validity: DURABLE_UPLOAD_SESSION_VALIDITY_TIME_SECS as u32,
        uploaded: 0,
    }))
}

pub async fn handle_upload_durable(
    session_id: Uuid,
    app_id: Uuid,
    bucket_id: Uuid,
    _accessor: BucketAccessor,
    reader: AbstractReadStream,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    trace!("Handle upload durable");
    let mut session = app_state
        .upload_manager
        .get_session(app_id, bucket_id, session_id)
        .await?;
    let bucket = get_bucket(app_id, bucket_id, &app_state.session).await?;
    trace!("Durable try lock {}", session.last_access);
    app_state
        .upload_manager
        .try_lock_session(
            &mut session,
            SessionState::AwaitingData,
            SessionState::Writing,
        )
        .await?;
    trace!("Durable lock acquired {}", session.last_access);

    let mut sorted_chunks: Vec<FileChunk> = session.fragments.clone().into_iter().collect();
    sorted_chunks.sort_by_key(|c| c.chunk_order);

    let mut futures = vec![];
    for chunk in &sorted_chunks {
        futures.push(query_chunk(chunk.chunk_id, chunk.server_id, &app_state));
    }
    let already_uploaded = try_join_all(futures)
        .await?
        .iter()
        .map(|item| item.unwrap_or(0) as i64)
        .sum();
    let split_path = split_path(&session.path);

    if already_uploaded == session.size {
        let file_id = session.file_id;
        return end_session(
            app_state,
            split_path,
            session.size,
            &mut session,
            bucket,
            (app_id, session_id),
            None,
            file_id,
        )
        .await;
    }

    let mut curr = 0i64;
    let mut i: i32 = -1;
    let mut skip = 0;
    for (frag, idx) in sorted_chunks.iter().zip(0..) {
        curr += frag.chunk_size;
        if curr > already_uploaded {
            i = idx;
            let uploaded_chunk_current = curr - already_uploaded;
            skip = frag.chunk_size - uploaded_chunk_current;
            break;
        }
    }

    if i == -1 {
        error!("FATAL: Something went very wrong with the durable file upload. curr={curr} already_uploaded={already_uploaded} session={session:?} chunks={sorted_chunks:?}");
        return Err(NodeClientError::InternalError);
    }

    let bucket_upload_session = Arc::new(Mutex::new(session));
    let notifier = create_commit_notifier(bucket_upload_session.clone(), app_state.clone());

    let transfer_result: NodeClientResponse<()> = async {
        let mut first = already_uploaded > 0; // Note handling user vs internal mdsftp_error would be nice.
        let mut skip = if first { skip as u64 } else { 0 };
        trace!("Transferring to {sorted_chunks:?} curr={i} skip={skip}");
        for chunk in sorted_chunks.iter().skip(i.try_into().unwrap()) {
            trace!("Durable inbound transfer {chunk:?} append={first}");
            inbound_transfer(
                reader.clone(),
                skip,
                chunk.server_id,
                chunk.chunk_id,
                None,
                ChunkInfo {
                    chunk_buffer: 0,
                    size: chunk.chunk_size as u64,
                    append: first,
                },
                &app_state,
            )
            .await?;
            first = false;
            skip = 0;
        }
        Ok(())
    }
    .await;

    let mut session = bucket_upload_session.lock().await;
    // We are getting the session BEFORE the .abort call to ensure it is not in the middle of
    // updating the sessions last_access
    notifier.abort();

    if transfer_result.is_err() {
        warn!("Durable upload interrupted");
        session.state = SessionState::TimedOut.into();
        app_state
            .upload_manager
            .update_session(&mut session)
            .await?;

        return Err(NodeClientError::BadRequest);
    }

    let file_id = session.file_id;
    end_session(
        app_state,
        split_path,
        session.size,
        &mut session,
        bucket,
        (app_id, session_id),
        None,
        file_id,
    )
    .await
}

pub async fn resume_upload_session(
    app_id: Uuid,
    bucket_id: Uuid,
    session_id: Uuid,
    app_state: Data<AppState>,
) -> NodeClientResponse<i64> {
    trace!("resume_upload_session call");
    let mut session = app_state
        .upload_manager
        .get_session(app_id, bucket_id, session_id)
        .await?;
    trace!("session obtained");
    app_state
        .upload_manager
        .try_lock_session(
            &mut session,
            SessionState::TimedOut,
            SessionState::AwaitingData,
        )
        .await?;
    trace!("session locked");

    let mut sorted_chunks: Vec<FileChunk> = session.fragments.clone().into_iter().collect();
    sorted_chunks.sort_by_key(|c| c.chunk_order);
    trace!("Resuming upload session");

    let mut futures = vec![];
    for chunk in &sorted_chunks {
        futures.push(query_chunk(chunk.chunk_id, chunk.server_id, &app_state));
    }

    trace!("chunks scheduled");
    Ok(try_join_all(futures)
        .await?
        .iter()
        .map(|item| item.unwrap_or(0) as i64)
        .sum())
}

#[allow(clippy::too_many_arguments)]
pub async fn end_session(
    app_state: Data<AppState>,
    split_path: (Option<String>, String),
    size: i64,
    bucket_upload_session: &mut BucketUploadSession,
    bucket: Bucket,
    app_session_ids: (Uuid, Uuid),
    old_file: Option<Option<File>>,
    file_id: Uuid,
) -> NodeClientResponse<()> {
    debug!("Committing chunks {:?}", &bucket_upload_session.fragments);
    let mut futures = vec![];
    for chunk in &bucket_upload_session.fragments {
        futures.push(commit_chunk(
            CommitFlags::r#final(),
            chunk.server_id,
            chunk.chunk_id,
            &app_state,
        ))
    }
    try_join_all(futures).await?;

    let now = Utc::now();
    let directory = if let Some(directory) = split_path.0 {
        try_mkdir(bucket.id, directory, &app_state.session)
            .await?
            .map(|dir| dir.id)
    } else {
        Some(ROOT_DIR)
    };

    let file = File {
        bucket_id: bucket.id,
        directory: if let Some(directory) = directory {
            directory
        } else {
            ROOT_DIR
        },
        name: split_path.1.clone(),
        id: file_id,
        size,
        chunk_ids: bucket_upload_session.fragments.clone(),
        created: now,
        last_modified: now,
    };
    let old_file = old_file.unwrap_or(
        get_file(bucket.id, directory, split_path.1, &app_state.session)
            .await
            .ok(),
    );

    if old_file.is_some() {
        do_delete_file(old_file.as_ref().unwrap(), &bucket, &app_state).await?;
        trace!("Deleted residual file");
    }

    trace!("Inserting file record");
    insert_file(&file, &bucket, &app_state.session).await?;
    app_state
        .upload_manager
        .end_session(app_session_ids.0, bucket.id, app_session_ids.1)
        .await;

    trace!("End session complete");

    Ok(())
}

pub async fn try_mkdir(
    bucket_id: Uuid,
    path: String,
    session: &CachingSession,
) -> NodeClientResponse<Option<Directory>> {
    if path.is_empty() {
        return Ok(None);
    }

    let (parent_path, dir_name) = split_path(&path);

    if let Ok(existing_dir) = get_directory(bucket_id, Some(path.clone()), session).await {
        return Ok(existing_dir);
    }

    let mut directories_to_create = Vec::new();

    let mut current_parent_path = (parent_path.clone(), String::new());
    let mut parent_path_buf =
        get_directory(bucket_id, current_parent_path.clone().0, session).await;

    while parent_path_buf.is_err() {
        current_parent_path = split_path(current_parent_path.clone().0.unwrap().as_str());
        directories_to_create.push(current_parent_path.clone());
        parent_path_buf = get_directory(bucket_id, current_parent_path.clone().0, session).await;
    }

    directories_to_create.push((parent_path, dir_name));

    let directories_len = directories_to_create.len();

    for (i, directory) in (0..).zip(directories_to_create) {
        let new_dir = Directory {
            bucket_id,
            parent: directory.0.unwrap_or_default(),
            name: directory.1,
            id: Uuid::new_v4(),
            created: Utc::now(),
            last_modified: Utc::now(),
        };

        insert_directory(&new_dir, session).await?;

        if i == directories_len - 1 {
            return Ok(Some(new_dir));
        }
    }

    unreachable!("Something went very wrong when creating the directory")
}

pub fn create_commit_notifier(
    bucket_upload_session: Arc<Mutex<BucketUploadSession>>,
    data: Data<AppState>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut futures = vec![];
            {
                let session = bucket_upload_session.lock().await;
                for chunk in &session.fragments {
                    futures.push(commit_chunk(
                        CommitFlags::keep_alive(),
                        chunk.server_id,
                        chunk.chunk_id,
                        &data,
                    ));
                }
            }
            let _ = try_join_all(futures).await;
            {
                let mut session = bucket_upload_session.lock().await;
                let _ = data.upload_manager.update_session(&mut session).await;
            }
        }
    })
}

pub async fn handle_download(
    e_path: EntryPath,
    accessor: BucketAccessor,
    writer: AbstractWriteStream,
    app_state: Data<AppState>,
    range: Option<ByteRangeSpec>,
) -> NodeClientResponse<(DlInfo, JoinHandle<NodeClientResponse<()>>)> {
    accessor.has_permission(&e_path.app_id, &e_path.bucket_id, *DOWNLOAD_ALLOWANCE)?;
    let path = split_path(&e_path.path());
    let attachment_name = path.1.clone();
    let file = get_file_dir(e_path.bucket_id, path.0, path.1, &app_state.session).await?;

    let range = if let Some(range) = range {
        // end inclusive
        Some(
            range
                .to_satisfiable_range(file.0.size as u64)
                .ok_or(NodeClientError::RangeUnsatisfiable)?,
        )
    } else {
        None
    };

    let mut chunk_ids: Vec<FileChunk> = file.0.chunk_ids.iter().cloned().collect();
    chunk_ids.sort_by_key(|chunk| chunk.chunk_order);
    let mut chunk_ranges: Vec<Option<ChunkRange>> =
        vec![Some(ChunkRange::default()); chunk_ids.len()];

    if let Some(range) = range {
        let mut start = 0i64;
        for (chunk, i) in chunk_ids.iter().zip(0..) {
            let end = start + chunk.chunk_size - 1;

            let in_range_start = max(start, range.0 as i64);
            let in_range_end = min(end, range.1 as i64);

            if in_range_end < in_range_start {
                chunk_ranges[i] = None;
            } else if in_range_end != end || in_range_start != start {
                chunk_ranges[i] = Some(ChunkRange::new(
                    (in_range_start - start) as u64,
                    (in_range_end + 1 - start) as u64,
                )?);
            } // else leave default

            start += chunk.chunk_size;
        }
    }

    let handle: JoinHandle<NodeClientResponse<()>> = tokio::spawn(async move {
        for (chunk, range) in chunk_ids.iter().zip(chunk_ranges) {
            if let Some(range) = range {
                outbound_transfer(
                    writer.clone(),
                    chunk.server_id,
                    chunk.chunk_id,
                    &app_state,
                    range.into_option(),
                )
                .await?;
            }
        }
        writer.lock().await.shutdown().await?;
        Ok(())
    });

    Ok((
        DlInfo {
            size: file.0.size as u64,
            mime: ContentType(
                mime_guess::from_path(&attachment_name).first_or(mime::APPLICATION_OCTET_STREAM),
            ),
            attachment_name,
        },
        handle,
    ))
}
