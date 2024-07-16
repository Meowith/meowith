use actix_web::{get, post, put, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::public::response::NodeClientResponse;

#[derive(Serialize)]
pub struct UploadSessionStartResponse {
    /// To be used in the X-UploadCode header
    code: String,
    /// Seconds till the unfinished chunk is dropped when the upload is not reinitialized
    validity: u32,
}

#[derive(Deserialize)]
pub struct UploadSessionRequest {
    /// Entry size in bytes
    size: u64,
    /// Entry full path
    name: String,
}

#[post("/upload/oneshot/{app_id}/{bucket_id}")]
pub async fn upload_oneshot(_path: web::Path<(Uuid, Uuid)>) -> NodeClientResponse<String> {
    todo!()
}

#[post("/upload/durable/{app_id}/{bucket_id}")]
pub async fn start_upload_durable(
    _path: web::Path<(Uuid, Uuid)>,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
    todo!()
}

#[put("/upload/put")]
pub async fn upload_durable() -> NodeClientResponse<String> {
    todo!()
}

#[get("/download/{app_id}/{bucket_id}")]
pub async fn download(_path: web::Path<(Uuid, Uuid)>) -> NodeClientResponse<String> {
    todo!()
}
