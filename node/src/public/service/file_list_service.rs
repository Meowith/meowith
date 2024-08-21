use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::routes::entity_list::{Entity, ListResponse};
use crate::public::routes::EntryPath;
use crate::public::service::{LIST_BUCKET_ALLOWANCE, LIST_DIR_ALLOWANCE};
use crate::AppState;
use actix_web::web;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::file_access::{
    get_directory, get_files_from_bucket, get_files_from_bucket_and_directory,
    get_files_from_bucket_and_directory_paginated, get_files_from_bucket_paginated, FileItem, DID,
};
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

pub async fn do_list_bucket(
    app_id: Uuid,
    bucket_id: Uuid,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    pagination_info: PaginationInfo,
) -> NodeClientResponse<ListResponse> {
    pagination_info.validate()?;
    accessor.has_permission(&bucket_id, &app_id, *LIST_BUCKET_ALLOWANCE)?;
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

pub async fn do_list_dir(
    mut e_path: EntryPath,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    pagination_info: PaginationInfo,
) -> NodeClientResponse<ListResponse> {
    pagination_info.validate()?;
    accessor.has_permission(&e_path.bucket_id, &e_path.app_id, *LIST_DIR_ALLOWANCE)?;

    let path = if e_path.path().is_empty() {
        None
    } else {
        Some(e_path.path())
    };

    let dir = get_directory(e_path.bucket_id, path, &app_data.session).await?;
    let files: Box<dyn Stream<Item = FileItem> + Unpin> = if pagination_info.is_paginated() {
        let complete = pagination_info.completed();
        Box::new(
            get_files_from_bucket_and_directory_paginated(
                e_path.bucket_id,
                DID::of(dir).0,
                &app_data.session,
                complete.start.unwrap(),
                complete.end.unwrap(),
            )
            .await?,
        )
    } else {
        Box::new(
            get_files_from_bucket_and_directory(
                e_path.bucket_id,
                dir.map(|dir| dir.id),
                &app_data.session,
            )
            .await?,
        )
    };

    //TODO append dirs
    collect_files(files, false).await
}

async fn collect_files(
    mut files: Box<dyn Stream<Item = FileItem> + Unpin>,
    include_dir: bool,
) -> NodeClientResponse<ListResponse> {
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

    Ok(ListResponse { entities })
}
