use crate::public::services::user_service::{do_get_all_users, do_update_role};
use crate::setup::auth_routes::EmptyResponse;
use crate::AppState;
use actix_web::web::Json;
use actix_web::{get, post, web};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::UserListDTO;
use data::model::permission_model::GlobalRole;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UserUpdateRoleRequest {
    pub role: GlobalRole,
}

#[get("/")]
pub async fn list_users(data: web::Data<AppState>) -> NodeClientResponse<Json<UserListDTO>> {
    do_get_all_users(&data.session).await
}

#[post("/update/{id}")]
pub async fn update_role(
    req: web::Path<Uuid>,
    request: Json<UserUpdateRoleRequest>,
    data: web::Data<AppState>,
) -> NodeClientResponse<Json<EmptyResponse>> {
    let user_id: Uuid = req.into_inner();

    do_update_role(user_id, request.role, &data.session)
        .await
        .map(Json)
}
