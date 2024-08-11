use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::NodeClientResponse;
use crate::public::service::file_list_service::{do_list_bucket, do_list_dir, PaginationInfo};
use crate::AppState;
use actix_web::{get, web};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ListResponse {
    pub entities: Vec<Entity>,
}

#[derive(Serialize)]
pub struct Entity {
    pub name: String,
    pub dir: String,
    pub size: u64,
    pub is_dir: bool,
    pub created: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

#[get("/list/bucket/{app_id}/{bucket_id}")]
pub async fn list_bucket(
    path: web::Path<(Uuid, Uuid, String)>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    paginate: web::Query<PaginationInfo>,
) -> NodeClientResponse<web::Json<ListResponse>> {
    do_list_bucket(path.0, path.1, accessor, app_data, paginate.0)
        .await
        .map(web::Json)
}

#[get("/list/dir/{app_id}/{bucket_id}/{path}")]
pub async fn list_dir(
    path: web::Path<(Uuid, Uuid, String)>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    paginate: web::Query<PaginationInfo>,
) -> NodeClientResponse<web::Json<ListResponse>> {
    do_list_dir(
        path.0,
        path.1,
        path.2.clone(),
        accessor,
        app_data,
        paginate.0,
    )
    .await
    .map(web::Json)
}
