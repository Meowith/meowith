use charybdis::operations::{Delete, Insert};
use charybdis::stream::CharybdisModelStream;
use futures::try_join;
use scylla::{CachingSession, QueryResult};
use uuid::Uuid;

use crate::error::MeowithDataError;
use crate::model::file_model::{Bucket, File};

pub async fn get_files_from_bucket(
    bucket_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<File>, MeowithDataError> {
    File::find_by_bucket_id(bucket_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
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
