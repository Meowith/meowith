use charybdis::stream::CharybdisModelStream;
use scylla::CachingSession;
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
        .map_err(|e| e.into())
}

pub async fn get_files_from_bucket_and_directory(
    bucket_id: Uuid,
    directory: String,
    session: &CachingSession,
) -> Result<CharybdisModelStream<File>, MeowithDataError> {
    File::find_by_bucket_id_and_directory(bucket_id, directory)
        .execute(session)
        .await
        .map_err(|e| e.into())
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
        .map_err(|e| e.into())
}

pub async fn get_bucket(
    app_id: Uuid,
    name: String,
    session: &CachingSession,
) -> Result<Bucket, MeowithDataError> {
    Bucket::find_first_by_app_id_and_name(app_id, name)
        .execute(session)
        .await
        .map_err(|e| e.into())
}
