use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::NodeClientResponse;
use crate::public::routes::entity_action::RenameEntityRequest;
use crate::public::routes::EntryPath;
use crate::public::service::DELETE_ALLOWANCE;
use crate::AppState;
use actix_web::web::Data;
use data::access::file_access::{
    delete_file, get_bucket, get_file_dir, maybe_get_file_dir, update_file_path, DID,
};
use data::model::file_model::{Bucket, File};
use data::pathlib::split_path;
use logging::log_err;
use tokio::try_join;

pub async fn delete_file_srv(
    mut path: EntryPath,
    bucket_accessor: BucketAccessor,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    bucket_accessor.has_permission(&path.bucket_id, &path.app_id, *DELETE_ALLOWANCE)?;
    let split_path = split_path(&path.path());
    let (bucket, file) = try_join!(
        get_bucket(path.app_id, path.bucket_id, &app_state.session),
        get_file_dir(
            path.bucket_id,
            split_path.0.clone(),
            split_path.1.clone(),
            &app_state.session
        )
    )?;
    do_delete_file(&file.0, &bucket, &app_state).await?;
    Ok(())
}

pub async fn do_delete_file(
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

pub async fn rename_file_srv(
    mut path: EntryPath,
    mut req: RenameEntityRequest,
    bucket_accessor: BucketAccessor,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    if path.path() == req.path() {
        // The paths equal, no work needs to be done
        return Ok(());
    }

    let split_old_path = split_path(&path.path());
    let split_new_path = split_path(&req.path());
    let (old_file, new_file) = try_join!(
        get_file_dir(
            path.bucket_id,
            split_old_path.0,
            split_old_path.1,
            &app_state.session
        ),
        maybe_get_file_dir(
            path.bucket_id,
            split_new_path.0.clone(),
            split_new_path.1.clone(),
            &app_state.session
        )
    )?;

    match new_file.0 {
        None => {
            // update the path
            let _new_file = update_file_path(
                &old_file.0,
                old_file.1.map(|dir| dir.id),
                split_new_path.1,
                &app_state.session,
            )
            .await?;
        }
        Some(new_file_file) => {
            // if a file already exists in the new destination, and the user possesses the required allowance, delete it
            bucket_accessor.has_permission(&path.bucket_id, &path.app_id, *DELETE_ALLOWANCE)?;
            let bucket = get_bucket(path.app_id, path.bucket_id, &app_state.session).await?;
            do_delete_file(&new_file_file, &bucket, &app_state).await?;
            let _new_file = update_file_path(
                &old_file.0,
                DID::of(new_file.1).0,
                split_new_path.1,
                &app_state.session,
            )
            .await?;
        }
    }

    Ok(())
}
