use crate::public::service::role_service::{
    do_create_role, do_delete_role, do_get_roles, do_patch_member_roles, do_patch_role,
};
use crate::AppState;
use actix_web::{delete, get, patch, post, web, HttpResponse};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::{
    AppRolePath, MemberIdRequest, MemberRoleRequest, ModifyRoleRequest, UserRoleResponse,
};
use data::model::user_model::User;
use uuid::Uuid;

#[get("/{app_id}")]
pub async fn get_roles(
    app_state: web::Data<AppState>,
    user: User,
    req: web::Path<Uuid>,
) -> NodeClientResponse<web::Json<UserRoleResponse>> {
    do_get_roles(req.into_inner(), user, &app_state.session).await
}

#[post("/{app_id}/{name}")]
pub async fn create_role(
    app_state: web::Data<AppState>,
    user: User,
    req: web::Path<AppRolePath>,
) -> NodeClientResponse<HttpResponse> {
    do_create_role(req.into_inner(), user, &app_state.session).await?;
    Ok(HttpResponse::Ok().finish())
}

#[delete("/{app_id}/{name}")]
pub async fn delete_role(
    app_state: web::Data<AppState>,
    user: User,
    req: web::Path<AppRolePath>,
) -> NodeClientResponse<HttpResponse> {
    do_delete_role(req.into_inner(), user, &app_state.session).await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/{app_id}/{name}")]
pub async fn modify_role(
    app_state: web::Data<AppState>,
    user: User,
    which: web::Path<AppRolePath>,
    req: web::Json<ModifyRoleRequest>,
) -> NodeClientResponse<HttpResponse> {
    do_patch_role(
        which.into_inner(),
        user,
        req.into_inner(),
        &app_state.session,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}

#[patch("/{app_id}/{id}/roles")]
pub async fn update_roles_for_member(
    app_state: web::Data<AppState>,
    user: User,
    req: web::Path<MemberIdRequest>,
    perms: web::Json<MemberRoleRequest>,
) -> NodeClientResponse<HttpResponse> {
    do_patch_member_roles(
        req.into_inner(),
        user,
        perms.into_inner(),
        &app_state.session,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
}
