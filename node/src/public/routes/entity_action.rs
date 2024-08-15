use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::NodeClientResponse;
use crate::public::routes::EntryPath;
use crate::public::service::directory_action_service::{do_create_directory, do_rename_directory};
use crate::public::service::file_action_service::{delete_file_srv, rename_file_srv};
use crate::AppState;
use actix_web::{delete, post, web, HttpResponse};
use data::pathlib::normalize;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct RenameEntityRequest {
    pub to: String,
    #[serde(skip)]
    cached_path: Option<String>,
}

impl RenameEntityRequest {
    pub fn path(&mut self) -> String {
        if let Some(val) = &self.cached_path {
            val.clone()
        } else {
            self.cached_path = Some(normalize(&self.to));
            self.cached_path.as_ref().unwrap().clone()
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct DeleteDirectoryRequest {
    #[allow(unused)]
    pub recursive: bool,
}

#[delete("/delete/file/{app_id}/{bucket_id}/{path}")]
pub async fn delete_file(
    path: web::Path<EntryPath>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    delete_file_srv(path.into_inner(), accessor, app_data).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/rename/file/{app_id}/{bucket_id}/{path}")]
pub async fn rename_file(
    path: web::Path<EntryPath>,
    req: web::Json<RenameEntityRequest>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    rename_file_srv(path.into_inner(), req.0, accessor, app_data).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/mkdir/{app_id}/{bucket_id}/{path}")]
pub async fn create_directory(
    path: web::Path<EntryPath>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    do_create_directory(path.into_inner(), accessor, app_data).await?;
    Ok(HttpResponse::Ok().finish())
}

#[delete("/delete/directory/{app_id}/{bucket_id}/{path}")]
pub async fn delete_directory(
    _path: web::Path<EntryPath>,
    _req: web::Path<DeleteDirectoryRequest>,
    _accessor: BucketAccessor,
    _app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    Ok(HttpResponse::Ok().finish())
}

#[post("/rename/directory/{app_id}/{bucket_id}/{path}")]
pub async fn rename_directory(
    path: web::Path<EntryPath>,
    req: web::Json<RenameEntityRequest>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    let path = path.into_inner();

    do_rename_directory(path, req.0, accessor, app_data).await?;

    Ok(HttpResponse::Ok().finish())
}
