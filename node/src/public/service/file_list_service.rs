use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::routes::EntryPath;
use crate::public::service::{LIST_BUCKET_ALLOWANCE, LIST_DIR_ALLOWANCE};
use crate::AppState;
use actix_web::web;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::file_access::{
    get_directories_from_bucket, get_directories_from_bucket_paginated, get_directory,
    get_files_from_bucket, get_files_from_bucket_and_directory, get_files_from_bucket_paginated,
    get_sub_dirs, DirectoryListItem, FileItem,
};
use data::dto::entity::{Entity, EntityList};
use data::error::MeowithDataError;
use futures::Stream;
use futures_util::StreamExt;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct PaginationInfo {
    pub start: Option<u64>,
    pub end: Option<u64>,
}

impl PaginationInfo {
    fn validate(&self) -> NodeClientResponse<()> {
        if self.is_paginated() {
            let valid = self.completed();
            if valid.start.unwrap() >= valid.end.unwrap() {
                Err(NodeClientError::BadRequest)
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn is_paginated(&self) -> bool {
        self.start.is_some() || self.end.is_some()
    }

    fn completed(&self) -> Self {
        PaginationInfo {
            start: Some(self.start.unwrap_or(0)),
            end: Some(self.start.unwrap_or(usize::MAX as u64)),
        }
    }
}

pub async fn do_list_bucket_files(
    app_id: Uuid,
    bucket_id: Uuid,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    pagination_info: PaginationInfo,
) -> NodeClientResponse<EntityList> {
    pagination_info.validate()?;
    accessor.has_permission(&app_id, &bucket_id, *LIST_BUCKET_ALLOWANCE)?;
    let files: Box<dyn Stream<Item = FileItem> + Unpin> = if pagination_info.is_paginated() {
        let complete = pagination_info.completed();
        Box::new(
            get_files_from_bucket_paginated(
                bucket_id,
                &app_data.session,
                complete.start.unwrap(),
                complete.end.unwrap(),
            )
            .await?,
        )
    } else {
        Box::new(get_files_from_bucket(bucket_id, &app_data.session).await?)
    };

    collect_files(files, true).await
}

pub async fn do_list_bucket_directories(
    app_id: Uuid,
    bucket_id: Uuid,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    pagination_info: PaginationInfo,
) -> NodeClientResponse<EntityList> {
    pagination_info.validate()?;
    accessor.has_permission(&app_id, &bucket_id, *LIST_BUCKET_ALLOWANCE)?;
    let mut sub_dirs: Box<dyn Stream<Item = DirectoryListItem> + Unpin> =
        if pagination_info.is_paginated() {
            let complete = pagination_info.completed();
            Box::new(
                get_directories_from_bucket_paginated(
                    bucket_id,
                    &app_data.session,
                    complete.start.unwrap(),
                    complete.end.unwrap(),
                )
                .await?,
            )
        } else {
            Box::new(get_directories_from_bucket(bucket_id, &app_data.session).await?)
        };

    let mut entries = Vec::new();
    while let Some(dir) = sub_dirs.next().await {
        let dir = dir.map_err(MeowithDataError::from)?;
        entries.push(Entity {
            name: dir.full_path(),
            dir: None,
            dir_id: Some(dir.id),
            size: 0,
            is_dir: true,
            created: dir.created,
            last_modified: dir.last_modified,
        })
    }
    Ok(EntityList { entities: entries })
}

pub async fn do_list_dir(
    mut e_path: EntryPath,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    pagination_info: PaginationInfo,
) -> NodeClientResponse<EntityList> {
    pagination_info.validate()?;
    accessor.has_permission(&e_path.app_id, &e_path.bucket_id, *LIST_DIR_ALLOWANCE)?;

    let path = if e_path.path().is_empty() {
        None
    } else {
        Some(e_path.path())
    };

    let dir = get_directory(e_path.bucket_id, path, &app_data.session).await?;

    let mut sub_dirs = get_sub_dirs(e_path.bucket_id, e_path.path(), &app_data.session).await?;

    let complete = pagination_info.completed();

    let mut length = complete
        .end
        .unwrap()
        .saturating_sub(complete.start.unwrap());

    let mut entries = Vec::new();
    while let Some(dir) = sub_dirs.next().await {
        if length == 0 {
            break;
        }
        let dir = dir.map_err(|_| NodeClientError::InternalError)?;

        entries.push(Entity {
            name: dir.name,
            dir: None,
            dir_id: Some(dir.id),
            size: 0,
            is_dir: true,
            created: dir.created,
            last_modified: dir.last_modified,
        });
        length -= 1;
    }

    if length > 0 {
        let files: Box<dyn Stream<Item = FileItem> + Unpin> = Box::new(
            get_files_from_bucket_and_directory(
                e_path.bucket_id,
                dir.map(|dir| dir.id),
                &app_data.session,
            )
            .await?,
        );
        for entity in collect_files(files, false).await?.entities {
            if length == 0 {
                break;
            }
            entries.push(entity);
            length -= 1;
        }
    }
    Ok(EntityList { entities: entries })
}

async fn collect_files(
    mut files: Box<dyn Stream<Item = FileItem> + Unpin>,
    include_dir: bool,
) -> NodeClientResponse<EntityList> {
    let mut entities = Vec::new();

    while let Some(item) = files.next().await {
        if let Ok(item) = item {
            entities.push(Entity {
                name: item.name,
                dir: if include_dir {
                    Some(item.directory)
                } else {
                    None
                },
                dir_id: None,
                size: item.size as u64,
                is_dir: false,
                created: Default::default(),
                last_modified: Default::default(),
            });
        }
    }

    Ok(EntityList { entities })
}
