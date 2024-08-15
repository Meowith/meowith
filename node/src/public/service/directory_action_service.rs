use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::public::routes::entity_action::RenameEntityRequest;
use crate::public::routes::EntryPath;
use crate::public::service::file_access_service::try_mkdir;
use crate::public::service::{CREATE_DIRECTORY_ALLOWANCE, RENAME_DIRECTORY_ALLOWANCE};
use crate::AppState;
use actix_web::web::Data;
use data::access::file_access::{get_directory, update_directory_path, DirectoryIterator};
use data::pathlib::{join_parent_name, normalize, split_path};
use futures::pin_mut;
use futures_util::StreamExt;

pub async fn do_create_directory(
    mut path: EntryPath,
    bucket_accessor: BucketAccessor,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    bucket_accessor.has_permission(&path.bucket_id, &path.app_id, *CREATE_DIRECTORY_ALLOWANCE)?;
    let _ = try_mkdir(path.bucket_id, path.path(), &app_state.session).await;
    Ok(())
}

pub async fn do_rename_directory(
    mut e_path: EntryPath,
    mut req: RenameEntityRequest,
    bucket_accessor: BucketAccessor,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    bucket_accessor.has_permission(
        &e_path.bucket_id,
        &e_path.app_id,
        *RENAME_DIRECTORY_ALLOWANCE,
    )?;
    let path = e_path.path();

    if path == req.path() {
        // The paths equal, no work needs to be done
        return Ok(());
    }

    if path.is_empty() || req.path().is_empty() {
        // no touching the root "dir"
        return Err(NodeClientError::BadRequest);
    }

    let original_directory = get_directory(e_path.bucket_id, Some(path), &app_state.session)
        .await?
        .unwrap(); // will not be None as it will not be the root dir.

    if get_directory(
        e_path.bucket_id,
        Some(req.path().clone()),
        &app_state.session,
    )
    .await
    .is_ok()
    {
        return Err(NodeClientError::EntityExists); // a directory with this name already exists
    }

    let (new_parent, new_name) = split_path(req.path().as_str());
    let new_parent = try_mkdir(
        e_path.bucket_id,
        new_parent.unwrap_or(String::new()),
        &app_state.session,
    )
    .await?;

    let child_stream =
        DirectoryIterator::from_parent(original_directory.clone(), &app_state.session);
    pin_mut!(child_stream);

    let mut to_rename = Vec::new();
    while let Some(res) = child_stream.next().await {
        to_rename.push(res?);
    }

    let old_parent_path = original_directory.full_path();
    let new_parent_path = join_parent_name(
        &new_parent
            .clone()
            .map(|d| d.full_path())
            .unwrap_or(String::new()),
        &new_name,
    );

    for dir in to_rename {
        let new_local_parent_path = normalize(
            (new_parent_path.clone() + "/" + &old_parent_path.clone()[old_parent_path.len()..])
                .as_str(),
        );
        let _new_dir = update_directory_path(
            &dir,
            Some(new_local_parent_path),
            dir.name.clone(),
            &app_state.session,
        )
        .await?;
    }

    let _new_dir = update_directory_path(
        &original_directory,
        new_parent.map(|d| d.full_path()),
        new_name,
        &app_state.session,
    )
    .await?;

    Ok(())
}
