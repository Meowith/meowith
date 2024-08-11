use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::{NodeClientError, NodeClientResponse};
use crate::public::routes::entity_list::{Entity, ListResponse};
use crate::public::service::{LIST_BUCKET_ALLOWANCE, LIST_DIR_ALLOWANCE};
use crate::AppState;
use actix_web::web;
use data::access::file_access::{
    get_files_from_bucket, get_files_from_bucket_and_directory,
    get_files_from_bucket_and_directory_paginated, get_files_from_bucket_paginated, FileItem,
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

    collect_files(files).await
}

pub async fn do_list_dir(
    app_id: Uuid,
    bucket_id: Uuid,
    path: String,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    pagination_info: PaginationInfo,
) -> NodeClientResponse<ListResponse> {
    pagination_info.validate()?;
    accessor.has_permission(&bucket_id, &app_id, *LIST_DIR_ALLOWANCE)?;

    let files: Box<dyn Stream<Item = FileItem> + Unpin> = if pagination_info.is_paginated() {
        let complete = pagination_info.completed();
        Box::new(
            get_files_from_bucket_and_directory_paginated(
                bucket_id,
                path,
                &app_data.session,
                complete.start.unwrap(),
                complete.end.unwrap(),
            )
            .await?,
        )
    } else {
        Box::new(get_files_from_bucket_and_directory(bucket_id, path, &app_data.session).await?)
    };

    collect_files(files).await
}

async fn collect_files(
    mut files: Box<dyn Stream<Item = FileItem> + Unpin>,
) -> NodeClientResponse<ListResponse> {
    let mut entities = Vec::new();

    while let Some(item) = files.next().await {
        if let Ok(item) = item {
            entities.push(Entity {
                name: item.name,
                dir: item.directory,
                size: item.size as u64,
                is_dir: false,
                created: Default::default(),
                last_modified: Default::default(),
            });
        }
    }

    Ok(ListResponse { entities })
}
