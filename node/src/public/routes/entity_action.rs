use crate::public::extractors::entry_path::EntryPath;
pub(crate) use crate::public::extractors::rename_request::RenameEntityRequest;
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::service::directory_action_service::{
    do_create_directory, do_delete_directory, do_rename_directory,
};
use crate::public::service::file_action_service::{delete_file_srv, rename_file_srv};
use crate::AppState;
use actix_web::{delete, post, web, HttpResponse};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::DeleteDirectoryRequest;

#[delete("/delete/{app_id}/{bucket_id}/{path:.*}")]
pub async fn delete_file(
    path: EntryPath,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    delete_file_srv(path, accessor, app_data).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/rename/{app_id}/{bucket_id}/{path:.*}")]
pub async fn rename_file(
    path: EntryPath,
    req: RenameEntityRequest,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    rename_file_srv(path, req, accessor, app_data).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/create/{app_id}/{bucket_id}/{path:.*}")]
pub async fn create_directory(
    path: EntryPath,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    do_create_directory(path, accessor, app_data).await?;
    Ok(HttpResponse::Ok().finish())
}

#[delete("/delete/{app_id}/{bucket_id}/{path:.*}")]
pub async fn delete_directory(
    path: EntryPath,
    req: web::Json<DeleteDirectoryRequest>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    do_delete_directory(path, req.0, accessor, app_data).await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/rename/{app_id}/{bucket_id}/{path:.*}")]
pub async fn rename_directory(
    path: EntryPath,
    req: RenameEntityRequest,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    do_rename_directory(path, req, accessor, app_data).await?;

    Ok(HttpResponse::Ok().finish())
}
