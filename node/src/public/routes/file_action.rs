use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::NodeClientResponse;
use crate::public::service::file_action_service::{delete_file_srv, rename_file_srv};
use crate::AppState;
use actix_web::{delete, web, HttpResponse, post};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct RenameFileRequest {
    pub to: String,
}

#[delete("/delete/{app_id}/{bucket_id}/{path}")]
pub async fn delete_file(
    path: web::Path<(Uuid, Uuid, String)>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    delete_file_srv(path.0, path.1, path.2.clone(), accessor, app_data).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/rename/{app_id}/{bucket_id}/{path}")]
pub async fn rename_file(
    path: web::Path<(Uuid, Uuid, String)>,
    req: web::Json<RenameFileRequest>,
    accessor: BucketAccessor,
    app_data: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    rename_file_srv(path.0, path.1, path.2.clone(), req.0, accessor, app_data).await?;
    Ok(HttpResponse::Ok().finish())
}
