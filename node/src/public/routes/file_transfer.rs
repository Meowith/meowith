use actix_web::{get, post, web};
use uuid::Uuid;

use crate::public::response::NodeClientResponse;

#[post("/upload/oneshot/{app_id}/{bucket_id}")]
pub async fn upload_oneshot(_path: web::Path<(Uuid, Uuid)>) -> NodeClientResponse<String> {
    todo!()
}

#[post("/upload/durable/{app_id}/{bucket_id}")]
pub async fn upload_durable(_path: web::Path<(Uuid, Uuid)>) -> NodeClientResponse<String> {
    todo!()
}

#[get("/download/{app_id}/{bucket_id}")]
pub async fn download(_path: web::Path<(Uuid, Uuid)>) -> NodeClientResponse<String> {
    todo!()
}
