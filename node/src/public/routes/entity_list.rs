use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::routes::EntryPath;
use crate::public::service::file_list_service::{do_list_bucket, do_list_dir, PaginationInfo};
use crate::AppState;
use actix_web::{get, web};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::EntityList;
use uuid::Uuid;

#[get("/list/bucket/{app_id}/{bucket_id}")]
pub async fn list_bucket(
    path: web::Path<(Uuid, Uuid, String)>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    paginate: web::Query<PaginationInfo>,
) -> NodeClientResponse<web::Json<EntityList>> {
    do_list_bucket(path.0, path.1, accessor, app_data, paginate.0)
        .await
        .map(web::Json)
}

#[get("/list/dir/{app_id}/{bucket_id}/{path}")]
pub async fn list_dir(
    path: web::Path<EntryPath>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
    paginate: web::Query<PaginationInfo>,
) -> NodeClientResponse<web::Json<EntityList>> {
    do_list_dir(path.into_inner(), accessor, app_data, paginate.0)
        .await
        .map(web::Json)
}
