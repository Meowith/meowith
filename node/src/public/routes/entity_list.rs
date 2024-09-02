use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::routes::EntryPath;
use crate::public::service::file_list_service::{
    do_list_bucket_directories, do_list_bucket_files, do_list_dir, PaginationInfo,
};
use crate::AppState;
use actix_web::{get, web};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::EntityList;
use uuid::Uuid;

#[get("/list/files/{app_id}/{bucket_id}")]
pub async fn list_bucket_files(
    path: web::Path<(Uuid, Uuid)>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    paginate: web::Query<PaginationInfo>,
) -> NodeClientResponse<web::Json<EntityList>> {
    do_list_bucket_files(path.0, path.1, accessor, app_data, paginate.0)
        .await
        .map(web::Json)
}

#[get("/list/directories/{app_id}/{bucket_id}")]
pub async fn list_bucket_directories(
    path: web::Path<(Uuid, Uuid)>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    paginate: web::Query<PaginationInfo>,
) -> NodeClientResponse<web::Json<EntityList>> {
    do_list_bucket_directories(path.0, path.1, accessor, app_data, paginate.0)
        .await
        .map(web::Json)
}

#[get("/list/{app_id}/{bucket_id}/{path:.*}")]
pub async fn list_directory(
    path: web::Path<EntryPath>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    paginate: web::Query<PaginationInfo>,
) -> NodeClientResponse<web::Json<EntityList>> {
    do_list_dir(path.into_inner(), accessor, app_data, paginate.0)
        .await
        .map(web::Json)
}
