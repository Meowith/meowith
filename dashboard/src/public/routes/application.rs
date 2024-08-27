use crate::public::service::application_service::{do_create_app, do_delete_app};
use crate::AppState;
use actix_web::{delete, post, web, HttpResponse};
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::dto::entity::AppDto;
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
