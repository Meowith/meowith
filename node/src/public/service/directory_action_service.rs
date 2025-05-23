use crate::public::extractors::entry_path::EntryPath;
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::routes::entity_action::RenameEntityRequest;
use crate::public::service::file_access_service::try_mkdir;
use crate::public::service::{
    CREATE_DIRECTORY_ALLOWANCE, DELETE_ALLOWANCE, RENAME_DIRECTORY_ALLOWANCE,
};
use crate::AppState;
use actix_web::web::Data;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::file_access::{
    delete_directory, get_directory, maybe_get_first_child_from_directory,
    maybe_get_first_file_from_directory, update_directory_path, DirectoryIterator,
};
use data::dto::entity::DeleteDirectoryRequest;
use data::model::file_model::Directory;
use data::pathlib::{join_parent_name, normalize, split_path};
use futures::pin_mut;
use futures_util::StreamExt;

pub async fn do_create_directory(
    path: EntryPath,
    bucket_accessor: BucketAccessor,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    bucket_accessor.has_permission(&path.app_id, &path.bucket_id, *CREATE_DIRECTORY_ALLOWANCE)?;
    let _ = try_mkdir(path.bucket_id, path.path(), &app_state.session).await?;
    Ok(())
}

pub async fn do_delete_directory(
    e_path: EntryPath,
    req: DeleteDirectoryRequest,
    bucket_accessor: BucketAccessor,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    bucket_accessor.has_permission(&e_path.app_id, &e_path.bucket_id, *DELETE_ALLOWANCE)?;
    let path = e_path.path();

    if path.is_empty() {
        return Err(NodeClientError::BadRequest);
    }

    let directory = get_directory(e_path.bucket_id, Some(path.clone()), &app_state.session)
        .await?
        .unwrap(); // will not be None as it will not be the root dir.

    if req.recursive {
        let child_stream = DirectoryIterator::from_parent(directory.clone(), &app_state.session);
        pin_mut!(child_stream);
        let mut children = Vec::new();

        while let Some(res) = child_stream.next().await {
            children.push(res?);
        }

        for dir in children {
            dir_empty(&directory, &app_state, false).await?;

            delete_directory(&dir, &app_state.session).await?;
        }
    } else {
        dir_empty(&directory, &app_state, true).await?;
    }

    delete_directory(&directory, &app_state.session).await?;

    Ok(())
}

async fn dir_empty(
    directory: &Directory,
    app_state: &Data<AppState>,
    check_dir: bool,
) -> NodeClientResponse<()> {
    if check_dir
        && maybe_get_first_child_from_directory(
            directory.bucket_id,
            Some(directory.full_path()),
            &app_state.session,
        )
        .await?
        .is_some()
    {
        return Err(NodeClientError::NotEmpty);
    }

    if maybe_get_first_file_from_directory(
        directory.bucket_id,
        Some(directory.id),
        &app_state.session,
    )
    .await?
    .is_some()
    {
        return Err(NodeClientError::NotEmpty);
    }

    Ok(())
}

pub async fn do_rename_directory(
    e_path: EntryPath,
    req: RenameEntityRequest,
    bucket_accessor: BucketAccessor,
    app_state: Data<AppState>,
) -> NodeClientResponse<()> {
    bucket_accessor.has_permission(
        &e_path.app_id,
        &e_path.bucket_id,
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
