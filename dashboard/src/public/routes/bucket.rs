use crate::public::service::bucket_service::{
    do_create_bucket, do_delete_bucket, do_edit_bucket, do_get_upload_sessions,
};
use crate::AppState;
use actix_web::{delete, get, patch, post, web, HttpResponse};
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::dto::entity::{BucketDto, UploadSessionsResponse};
use data::model::user_model::User;
use log::info;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct CreateBucketRequest {
    pub name: String,
    pub app_id: Uuid,
    pub quota: u64,
    pub atomic_upload: bool,
}

#[derive(Serialize, Deserialize)]
pub struct EditBucketQuotaRequest {
    pub quota: u64,
}

impl CreateBucketRequest {
    fn validate(&self) -> NodeClientResponse<()> {
        if self.name.len() < 3 || self.name.len() > 64 {
            return Err(NodeClientError::BadRequest);
        }
        Ok(())
    }
}

#[post("/create")]
pub async fn create_bucket(
    app_state: web::Data<AppState>,
    req: web::Json<CreateBucketRequest>,
    user: User,
) -> NodeClientResponse<web::Json<BucketDto>> {
    req.validate()?;
    do_create_bucket(app_state, req.0, user).await
}

#[derive(Serialize, Deserialize)]
pub struct DelReq {
    pub app_id: Uuid,
    pub bucket_id: Uuid,
}

#[delete("/delete")]
pub async fn delete_bucket_handler(
    path: web::Json<DelReq>,
    app_state: web::Data<AppState>,
    user: User,
) -> NodeClientResponse<HttpResponse> {
    info!("Deleting bucket");
    let params = path.into_inner();
    do_delete_bucket(&app_state.session, params.app_id, params.bucket_id, user).await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/update/{app_id}/{bucket_id}")]
pub async fn edit_bucket(
    app_state: web::Data<AppState>,
    which: web::Path<(Uuid, Uuid)>,
    req: web::Json<EditBucketQuotaRequest>,
    user: User,
) -> NodeClientResponse<HttpResponse> {
    do_edit_bucket(&app_state.session, req.into_inner(), which.0, which.1, user).await?;
    Ok(HttpResponse::Ok().finish())
}

#[get("/sessions/{app_id}/{bucket_id}")]
pub async fn get_sessions(
    app_state: web::Data<AppState>,
    which: web::Path<(Uuid, Uuid)>,
    user: User,
) -> NodeClientResponse<web::Json<UploadSessionsResponse>> {
    do_get_upload_sessions(&app_state.session, which.1, which.0, user).await
}
