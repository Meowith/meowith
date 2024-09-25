use crate::public::service::user_service::{do_get_user_by_id, do_get_user_by_name};
use crate::AppState;
use actix_web::{get, web};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::MaybeUserDTO;
use uuid::Uuid;

#[get("/lookup/name/{name}")]
pub async fn user_by_name(
    req: web::Path<String>,
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<MaybeUserDTO>> {
    do_get_user_by_name(req.into_inner(), &state.session).await
}

#[get("/lookup/id/{id}")]
pub async fn user_by_id(
    req: web::Path<Uuid>,
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<MaybeUserDTO>> {
    do_get_user_by_id(req.into_inner(), &state.session).await
}
