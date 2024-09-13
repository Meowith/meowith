use async_stream::stream;
use charybdis::batch::ModelBatch;
use charybdis::errors::CharybdisError;
use charybdis::operations::{Delete, Insert, Update};
use charybdis::stream::CharybdisModelStream;
use charybdis::types::Timestamp;
use chrono::Utc;
use futures::stream::Skip;
use futures::stream::Take;
use futures::{try_join, Stream, StreamExt, TryFutureExt};
use log::error;
use scylla::query::Query;
use scylla::{CachingSession, IntoTypedRows, QueryResult};
use std::collections::VecDeque;
use uuid::Uuid;

pub const ROOT_DIR: Uuid = Uuid::from_u128(0);

use crate::error::MeowithDataError;
use crate::model::file_model::{
    update_bucket_query, Bucket, BucketUploadSession, Directory, File, UpdateBucketUploadSession,
};
use crate::pathlib::split_path;

pub type FileItem = Result<File, CharybdisError>;
pub type BucketItem = Result<Bucket, CharybdisError>;
pub type DirectoryItem = Result<Directory, MeowithDataError>;
pub type DirectoryListItem = Result<Directory, CharybdisError>;
pub type FileDir = (File, Option<Directory>);
pub type MaybeFileDir = (Option<File>, Option<Directory>);

/// Mapper for directory ids
/// DID::of(directory).0
/// expr.into::<DID>()
pub struct DID(pub Option<Uuid>);

impl DID {
    pub fn of(dir: Option<Directory>) -> Self {
        DID(dir.map(|d| d.id))
    }
}

impl From<Option<Directory>> for DID {
    fn from(value: Option<Directory>) -> Self {
        DID::of(value)
    }
}

pub struct DirectoryIterator<'a> {
    session: &'a CachingSession,
    visit_queue: VecDeque<Directory>,
    charybdis_model_stream: Option<CharybdisModelStream<Directory>>,
}

impl<'a> DirectoryIterator<'a> {
    pub fn from_parent(
        parent: Directory,
        session: &'a CachingSession,
    ) -> impl Stream<Item = DirectoryItem> + 'a {
        let mut queue = VecDeque::new();
        queue.push_front(parent);
        let mut iterator = DirectoryIterator {
            session,
            visit_queue: queue,
            charybdis_model_stream: None,
        };

        stream! {
            loop {
                if let Some(stream) = iterator.charybdis_model_stream.as_mut() {
                    match stream.next().await {
                        None => {
                            let _ = iterator.charybdis_model_stream.take();
                            continue;
                        },
                        Some(Ok(dir)) => {
                            iterator.visit_queue.push_back(dir.clone());
                            yield Ok(dir);
                        },
                        Some(Err(err)) => {
                            let _ = iterator.charybdis_model_stream.take();
                            yield Err(MeowithDataError::from(err));
                            return;
                        }
                    }
                } else if let Some(next) = iterator.visit_queue.pop_front() {
                    let stream = get_sub_dirs(next.bucket_id, next.full_path(), iterator.session).await;
                    match stream {
                        Ok(stream) => iterator.charybdis_model_stream = Some(stream),
                        Err(err) => {
                            yield Err(err);
                            return;
                        }
                    }
                } else {
                    return;
                }
            }
        }
    }
}

pub async fn get_directory(
    bucket_id: Uuid,
    path: Option<String>,
    session: &CachingSession,
) -> Result<Option<Directory>, MeowithDataError> {
    match path {
        None => Ok(None),
        Some(path) => {
            let path = split_path(&path);

            Directory::find_by_bucket_id_and_parent_and_name(
                bucket_id,
                path.0.unwrap_or("".to_string()),
                path.1,
            )
            .execute(session)
            .await
            .map_err(MeowithDataError::from)
            .map(Some)
        }
    }
}

pub async fn insert_bucket(
    bucket: &Bucket,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    bucket
        .insert()
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn maybe_get_first_bucket(
    app_id: Uuid,
    session: &CachingSession,
) -> Result<Option<Bucket>, MeowithDataError> {
    Bucket::maybe_find_first_by_app_id(app_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_buckets(
    app_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<Bucket>, MeowithDataError> {
    Bucket::find_by_app_id(app_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn insert_directory(
    directory: &Directory,
    session: &CachingSession,
) -> Result<(), MeowithDataError> {
    let _ = directory
        .insert()
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?;

    Ok(())
}

pub async fn get_directories_from_bucket(
    bucket_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<Directory>, MeowithDataError> {
    Directory::find_by_bucket_id(bucket_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_directories_from_bucket_paginated(
    bucket_id: Uuid,
    session: &CachingSession,
    start: u64,
    end: u64,
) -> Result<Take<Skip<CharybdisModelStream<Directory>>>, MeowithDataError> {
    Ok(Directory::find_by_bucket_id(bucket_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?
        .skip(start as usize)
        .take((end - start) as usize))
}

pub async fn get_files_from_bucket(
    bucket_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<File>, MeowithDataError> {
    File::find_by_bucket_id(bucket_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

// Note: rewrite this when the driver will support proper paging.
pub async fn get_files_from_bucket_paginated(
    bucket_id: Uuid,
    session: &CachingSession,
    start: u64,
    end: u64,
) -> Result<Take<Skip<CharybdisModelStream<File>>>, MeowithDataError> {
    Ok(File::find_by_bucket_id(bucket_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?
        .skip(start as usize)
        .take((end - start) as usize))
}

pub async fn get_files_from_bucket_and_directory(
    bucket_id: Uuid,
    directory: Option<Uuid>,
    session: &CachingSession,
) -> Result<CharybdisModelStream<File>, MeowithDataError> {
    File::find_by_bucket_id_and_directory(bucket_id, directory.unwrap_or(ROOT_DIR))
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_files_from_bucket_and_directory_paginated(
    bucket_id: Uuid,
    directory: Option<Uuid>,
    session: &CachingSession,
    start: u64,
    end: u64,
) -> Result<Take<Skip<CharybdisModelStream<File>>>, MeowithDataError> {
    Ok(
        File::find_by_bucket_id_and_directory(bucket_id, directory.unwrap_or(ROOT_DIR))
            .execute(session)
            .await
            .map_err(MeowithDataError::from)?
            .skip(start as usize)
            .take((end - start) as usize),
    )
}

pub async fn maybe_get_first_file_from_directory(
    bucket_id: Uuid,
    directory: Option<Uuid>,
    session: &CachingSession,
) -> Result<Option<File>, MeowithDataError> {
    File::maybe_find_first_by_bucket_id_and_directory(bucket_id, directory.unwrap_or(ROOT_DIR))
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn maybe_get_first_child_from_directory(
    bucket_id: Uuid,
    directory: Option<String>,
    session: &CachingSession,
) -> Result<Option<Directory>, MeowithDataError> {
    Directory::maybe_find_first_by_bucket_id_and_parent(bucket_id, directory.unwrap_or_default())
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_sub_dirs(
    bucket_id: Uuid,
    path: String,
    session: &CachingSession,
) -> Result<CharybdisModelStream<Directory>, MeowithDataError> {
    Directory::find_by_bucket_id_and_parent(bucket_id, path)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_file(
    bucket_id: Uuid,
    directory: Option<Uuid>,
    name: String,
    session: &CachingSession,
) -> Result<File, MeowithDataError> {
    File::find_by_bucket_id_and_directory_and_name(bucket_id, directory.unwrap_or(ROOT_DIR), name)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_file_dir(
    bucket_id: Uuid,
    directory: Option<String>,
    name: String,
    session: &CachingSession,
) -> Result<FileDir, MeowithDataError> {
    let directory = get_directory(bucket_id, directory, session)
        .await
        .map_err(MeowithDataError::from)?;
    let id: Uuid;
    if let Some(directory) = &directory {
        id = directory.id;
    } else {
        id = ROOT_DIR;
    }
    let file = File::find_by_bucket_id_and_directory_and_name(bucket_id, id, name)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?;
    Ok((file, directory))
}

pub async fn maybe_get_file_dir(
    bucket_id: Uuid,
    directory: Option<String>,
    name: String,
    session: &CachingSession,
) -> Result<MaybeFileDir, MeowithDataError> {
    let directory = get_directory(bucket_id, directory, session)
        .await
        .map_err(MeowithDataError::from)?;
    let id: Uuid;
    if let Some(directory) = &directory {
        id = directory.id;
    } else {
        id = ROOT_DIR;
    }
    let file = File::maybe_find_first_by_bucket_id_and_directory_and_name(bucket_id, id, name)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?;
    Ok((file, directory))
}

pub async fn update_file_path(
    file: &File,
    directory: Option<Uuid>,
    name: String,
    session: &CachingSession,
) -> Result<File, MeowithDataError> {
    let mut new_file = file.clone();
    new_file.directory = directory.unwrap_or(Uuid::from_u128(0));
    new_file.name = name;
    new_file.last_modified = Utc::now();
    // There is no other way to do this as
    // the primary key ((bucket_id), directory, name) is immutable,
    // and is what we are changing.
    // We need to delete the old file and create a new one instead.
    // A batch operation within the same table and partition will be atomic.
    File::batch()
        .append_delete(file)
        .append_insert(&new_file)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?;
    Ok(new_file)
}

// Same deal here, as with the file path
pub async fn update_directory_path(
    directory: &Directory,
    parent: Option<String>,
    name: String,
    session: &CachingSession,
) -> Result<Directory, MeowithDataError> {
    let mut new_dir = directory.clone();
    new_dir.parent = parent.unwrap_or_default();
    new_dir.name = name;
    Directory::batch()
        .append_delete(directory)
        .append_insert(&new_dir)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?;
    Ok(new_dir)
}

pub async fn delete_directory(
    directory: &Directory,
    session: &CachingSession,
) -> Result<(), MeowithDataError> {
    let _ = directory
        .delete()
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?;
    Ok(())
}

pub async fn delete_file(
    file: &File,
    bucket: &Bucket,
    session: &CachingSession,
) -> Result<(), MeowithDataError> {
    let _ = try_join!(
        file.delete()
            .execute(session)
            .map_err(MeowithDataError::from),
        update_bucket_space(bucket.clone(), 1, file.size, session)
    )?;

    Ok(())
}

pub async fn get_bucket_by_name(
    app_id: Uuid,
    name: String,
    session: &CachingSession,
) -> Result<Bucket, MeowithDataError> {
    Bucket::find_first_by_app_id_and_name(app_id, name)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_bucket(
    app_id: Uuid,
    id: Uuid,
    session: &CachingSession,
) -> Result<Bucket, MeowithDataError> {
    Bucket::find_first_by_app_id_and_id(app_id, id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn delete_bucket(
    bucket: &Bucket,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    bucket
        .delete()
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

const QUERY_ATTEMPTS: u16 = 2048;

pub async fn update_bucket_space(
    mut bucket: Bucket,
    file_count_delta: i64,
    space_taken_delta: i64,
    session: &CachingSession,
) -> Result<(), MeowithDataError> {
    // TODO a second table + batch would be nicer
    let update_query = concat!(
        update_bucket_query!("file_count = ?, space_taken = ?"),
        " IF file_count = ? and space_taken = ?"
    );

    for _ in 0..QUERY_ATTEMPTS {
        let query = Query::new(update_query);

        let result = session
            .execute_unpaged(
                query,
                (
                    bucket.file_count + file_count_delta,
                    bucket.space_taken + space_taken_delta,
                    bucket.app_id,
                    bucket.id,
                    bucket.file_count,
                    bucket.space_taken,
                ),
            )
            .await?;

        if let Some(rows) = result.rows {
            if let Some(row) = rows.into_typed::<(bool, i64, i64)>().next() {
                let (applied, file_count, space_taken) =
                    row.map_err(|_| MeowithDataError::UnknownFailure)?;
                if applied {
                    return Ok(());
                } else {
                    bucket.space_taken = space_taken;
                    bucket.file_count = file_count;
                }
            }
        }
    }

    error!(
        "Update bucket query failed after {} attempts. {:?}",
        QUERY_ATTEMPTS, bucket
    );
    Err(MeowithDataError::UnknownFailure)
}

pub async fn insert_file(
    file: &File,
    bucket: &Bucket,
    session: &CachingSession,
) -> Result<(), MeowithDataError> {
    let _ = try_join!(
        file.insert()
            .execute(session)
            .map_err(MeowithDataError::from),
        update_bucket_space(bucket.clone(), 1, file.size, session)
    )?;

    Ok(())
}

pub async fn insert_upload_session(
    bucket_upload_session: &BucketUploadSession,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    bucket_upload_session
        .insert()
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_upload_session(
    app_id: Uuid,
    bucket: Uuid,
    id: Uuid,
    session: &CachingSession,
) -> Result<BucketUploadSession, MeowithDataError> {
    BucketUploadSession::find_first_by_app_id_and_bucket_and_id(app_id, bucket, id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_upload_sessions(
    app_id: Uuid,
    bucket: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<BucketUploadSession>, MeowithDataError> {
    BucketUploadSession::find_by_app_id_and_bucket(app_id, bucket)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn delete_upload_session(
    update_bucket_upload_session: &UpdateBucketUploadSession,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    update_bucket_upload_session
        .delete()
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn delete_upload_session_by(
    app_id: Uuid,
    bucket_id: Uuid,
    id: Uuid,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    BucketUploadSession::delete_by_app_id_and_bucket_and_id(app_id, bucket_id, id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn update_upload_session_last_access(
    app_id: Uuid,
    bucket: Uuid,
    id: Uuid,
    last_access: Timestamp,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    let upload_session_update = UpdateBucketUploadSession {
        app_id,
        bucket,
        id,
        last_access,
    };

    upload_session_update
        .update()
        .execute(session)
        .await
        .map_err(|e| e.into())
}
