use charybdis::batch::ModelBatch;
use charybdis::operations::{Delete, Insert, Update};
use charybdis::stream::CharybdisModelStream;
use charybdis::types::Timestamp;
use futures::stream::Skip;
use futures::stream::Take;
use futures::{try_join, StreamExt};
use scylla::{CachingSession, QueryResult};
use uuid::Uuid;

use crate::error::MeowithDataError;
use crate::model::file_model::{Bucket, BucketUploadSession, File, UpdateBucketUploadSession};

pub type FileItem = Result<File, charybdis::errors::CharybdisError>;

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
    directory: String,
    session: &CachingSession,
) -> Result<CharybdisModelStream<File>, MeowithDataError> {
    File::find_by_bucket_id_and_directory(bucket_id, directory)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_files_from_bucket_and_directory_paginated(
    bucket_id: Uuid,
    directory: String,
    session: &CachingSession,
    start: u64,
    end: u64,
) -> Result<Take<Skip<CharybdisModelStream<File>>>, MeowithDataError> {
    Ok(File::find_by_bucket_id_and_directory(bucket_id, directory)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?
        .skip(start as usize)
        .take((end - start) as usize))
}

pub async fn get_file(
    bucket_id: Uuid,
    directory: String,
    name: String,
    session: &CachingSession,
) -> Result<File, MeowithDataError> {
    File::find_by_bucket_id_and_directory_and_name(bucket_id, directory, name)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn maybe_get_file(
    bucket_id: Uuid,
    directory: String,
    name: String,
    session: &CachingSession,
) -> Result<Option<File>, MeowithDataError> {
    File::maybe_find_first_by_bucket_id_and_directory_and_name(bucket_id, directory, name)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn update_file_path(
    file: &File,
    directory: String,
    name: String,
    session: &CachingSession,
) -> Result<File, MeowithDataError> {
    let mut new_file = file.clone();
    new_file.directory = directory;
    new_file.name = name;
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

pub async fn delete_file(
    file: &File,
    bucket: &Bucket,
    session: &CachingSession,
) -> Result<(), MeowithDataError> {
    let _ = try_join!(
        file.delete().execute(session),
        bucket.decrement_space_taken(file.size).execute(session),
        bucket.decrement_file_count(1).execute(session),
    )
    .map_err(MeowithDataError::from)?;
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

pub async fn update_bucket_space(
    bucket: Bucket,
    file_count_delta: i64,
    space_taken_delta: i64,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    bucket
        .increment_space_taken(space_taken_delta)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)?;

    bucket
        .increment_file_count(file_count_delta)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn insert_file(
    file: &File,
    bucket: &Bucket,
    session: &CachingSession,
) -> Result<(), MeowithDataError> {
    let _ = try_join!(
        file.insert().execute(session),
        bucket.increment_space_taken(file.size).execute(session),
        bucket.increment_file_count(1).execute(session),
    )
    .map_err(MeowithDataError::from)?;

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
