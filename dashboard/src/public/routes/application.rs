use crate::public::service::application_service::{
    do_add_member, do_create_app, do_delete_app, do_delete_member, do_list_apps, do_list_buckets,
    do_list_members,
};
use crate::AppState;
use actix_web::{delete, get, post, web, HttpResponse};
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::dto::entity::{AppDto, AppList, BucketList, MemberIdRequest, MemberListDTO};
use data::model::user_model::User;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct CreateApplicationRequest {
    pub name: String,
}

impl CreateApplicationRequest {
    pub fn validate(&self) -> NodeClientResponse<()> {
        if self.name.len() < 3 || self.name.len() > 64 {
            return Err(NodeClientError::BadRequest);
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct DeleteApplicationRequest {
    id: Uuid,
}

#[get("/list")]
pub async fn list_owned(
    user: User,
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<AppList>> {
    do_list_apps(user, &state.session).await
}

#[get("/buckets/{id}")]
pub async fn buckets(
    user: User,
    path: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<BucketList>> {
    do_list_buckets(path.into_inner(), user, &state.session).await
}

#[post("/create")]
pub async fn create_application(
    req: web::Json<CreateApplicationRequest>,
    state: web::Data<AppState>,
    user: User,
) -> NodeClientResponse<web::Json<AppDto>> {
    req.validate()?;
    do_create_app(req.0, &state.session, user, &state.global_config).await
}

#[delete("/delete")]
pub async fn delete_application(
    req: web::Json<DeleteApplicationRequest>,
    state: web::Data<AppState>,
    user: User,
) -> NodeClientResponse<HttpResponse> {
    do_delete_app(req.id, &state.session, user).await?;
    Ok(HttpResponse::Ok().finish())
}

#[post("/{app_id}/member/{id}")]
pub async fn add_member(
    req: web::Path<MemberIdRequest>,
    state: web::Data<AppState>,
    user: User,
) -> NodeClientResponse<HttpResponse> {
    do_add_member(req.id, req.app_id, &state.session, user).await?;
    Ok(HttpResponse::Ok().finish())
}

#[delete("/{app_id}/member/{id}")]
pub async fn delete_member(
    req: web::Path<MemberIdRequest>,
    state: web::Data<AppState>,
    user: User,
) -> NodeClientResponse<HttpResponse> {
    do_delete_member(req.id, req.app_id, &state.session, user).await?;
    Ok(HttpResponse::Ok().finish())
}

#[get("/{app_id}/members")]
pub async fn list_members(
    req_path: web::Path<Uuid>,
    state: web::Data<AppState>,
    user: User,
) -> NodeClientResponse<web::Json<MemberListDTO>> {
    do_list_members(req_path.into_inner(), user, &state.session).await
}
