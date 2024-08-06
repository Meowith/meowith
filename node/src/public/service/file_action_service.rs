use actix_web::web;
use tokio::try_join;
use uuid::Uuid;

use data::access::file_access::{get_bucket, get_file, maybe_get_file, update_file_path};
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::NodeClientResponse;
use crate::public::service::file_access_service::{do_delete_file, split_path};
use crate::public::service::DELETE_ALLOWANCE;
use crate::AppState;
use crate::public::routes::file_action::RenameFileRequest;

pub async fn delete_file_srv(
    app_id: Uuid,
    bucket_id: Uuid,
    path: String,
    bucket_accessor: BucketAccessor,
    app_state: web::Data<AppState>,
) -> NodeClientResponse<()> {
    bucket_accessor.has_permission(&bucket_id, &app_id, *DELETE_ALLOWANCE)?;
    let split_path = split_path(path);
    let (bucket, file) = try_join!(
        get_bucket(app_id, bucket_id, &app_state.session),
        get_file(
            bucket_id,
            split_path.0.clone(),
            split_path.1.clone(),
            &app_state.session
        )
    )?;
    do_delete_file(&file, &bucket, &app_state).await?;
    Ok(())
}

pub async fn rename_file_srv(
    app_id: Uuid,
    bucket_id: Uuid,
    path: String,
    req: RenameFileRequest,
    bucket_accessor: BucketAccessor,
    app_state: web::Data<AppState>,
) -> NodeClientResponse<()> {
    let split_old_path = split_path(path);
    let split_new_path = split_path(req.to);
    let (old_file, new_file) = try_join!(
        get_file(
            bucket_id,
            split_old_path.0,
            split_old_path.1,
            &app_state.session
        ),
        maybe_get_file(
            bucket_id,
            split_new_path.0.clone(),
            split_new_path.1.clone(),
            &app_state.session
        )
    )?;

    match new_file {
        None => {
            // update the path
            let _new_file = update_file_path(&old_file, split_new_path.0, split_new_path.1, &app_state.session).await?;
        }
        Some(new_file) => {
            // if a file already exists in the new destination, and the user possesses the required allowance, delete it
            bucket_accessor.has_permission(&bucket_id, &app_id, *DELETE_ALLOWANCE)?;
            let bucket = get_bucket(app_id, bucket_id, &app_state.session).await?;
            do_delete_file(&new_file, &bucket, &app_state).await?;
            let _new_file = update_file_path(&old_file, split_new_path.0, split_new_path.1, &app_state.session).await?;
        }
    }

    Ok(())
}