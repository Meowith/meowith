use std::convert::Into;
use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::NodeClientResponse;
use actix_web::{get, post, put, web, HttpResponse};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use commons::permission::PermissionList;
use data::model::permission_model::UserPermission;

lazy_static! {
    static ref UPLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Write]).into();
    static ref DOWNLOAD_ALLOWANCE: u64 = PermissionList(vec![UserPermission::Read]).into();
}

#[derive(Serialize)]
#[allow(unused)]
pub struct UploadSessionStartResponse {
    /// To be used in the X-UploadCode header
    code: String,
    /// Seconds till the unfinished chunk is dropped when the upload is not reinitialized
    validity: u32,
}

#[derive(Deserialize)]
#[allow(unused)]
pub struct UploadSessionRequest {
    /// Entry size in bytes
    size: u64,
    /// Entry full path
    name: String,
}

#[post("/upload/oneshot/{app_id}/{bucket_id}")]
pub async fn upload_oneshot(
    path: web::Path<(Uuid, String)>,
    accessor: BucketAccessor,
    _body: web::Payload,
) -> NodeClientResponse<HttpResponse> {
    accessor.has_permission(&path.1, &path.0, *UPLOAD_ALLOWANCE)?;
    todo!()
}

#[post("/upload/durable/{app_id}/{bucket_id}")]
pub async fn start_upload_durable(
    path: web::Path<(Uuid, String)>,
    accessor: BucketAccessor,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    accessor.has_permission(&path.1, &path.0, *UPLOAD_ALLOWANCE)?;
    todo!()
}

#[put("/upload/put/{session_id}")]
pub async fn upload_durable(
    _path: web::Path<Uuid>,
    _accessor: BucketAccessor,
    _body: web::Payload,
) -> NodeClientResponse<HttpResponse> {
    todo!()
}

#[get("/download/{app_id}/{bucket_id}")]
pub async fn download(
    path: web::Path<(Uuid, String)>,
    accessor: BucketAccessor,
) -> NodeClientResponse<HttpResponse> {
    accessor.has_permission(&path.1, &path.0, *DOWNLOAD_ALLOWANCE)?;
    todo!()
}
