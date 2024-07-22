use crate::public::middleware::user_middleware::BucketAccessor;
use crate::public::response::NodeClientResponse;
use actix_web::{get, post, put, web, HttpResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    _path: web::Path<(Uuid, Uuid)>,
    _accessor: BucketAccessor,
    _body: web::Payload,
) -> NodeClientResponse<HttpResponse> {
    todo!()
}

#[post("/upload/durable/{app_id}/{bucket_id}")]
pub async fn start_upload_durable(
    _path: web::Path<(Uuid, Uuid)>,
    _accessor: BucketAccessor,
) -> NodeClientResponse<web::Json<UploadSessionStartResponse>> {
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
    _path: web::Path<(Uuid, Uuid)>,
    _accessor: BucketAccessor,
) -> NodeClientResponse<HttpResponse> {
    todo!()
}
