use crate::setup::auth_routes::EmptyResponse;
use actix_web::web;
use commons::error::std_response::NodeClientResponse;
use data::access::user_access::{get_all_users, update_user_quota, update_user_role};
use data::dto::entity::UserListDTO;
use data::error::MeowithDataError;
use data::model::permission_model::GlobalRole;
use data::model::user_model::User;
use futures_util::TryStreamExt;
use scylla::CachingSession;
use uuid::Uuid;

pub async fn do_get_all_users(
    session: &CachingSession,
) -> NodeClientResponse<web::Json<UserListDTO>> {
    let users: Vec<User> = get_all_users(session)
        .await?
        .try_collect()
        .await
        .map_err(MeowithDataError::from)?;
    Ok(web::Json(UserListDTO {
        users: users.into_iter().map(|x| x.into()).collect(),
    }))
}

pub async fn do_update_role(
    user_id: Uuid,
    role: GlobalRole,
    session: &CachingSession,
) -> NodeClientResponse<EmptyResponse> {
    update_user_role(user_id, role, session)
        .await
        .map_err(MeowithDataError::from)?;

    Ok(EmptyResponse)
}

pub async fn do_update_quota(
    user_id: Uuid,
    quota: u64,
    session: &CachingSession,
) -> NodeClientResponse<EmptyResponse> {
    update_user_quota(user_id, quota, session)
        .await
        .map_err(MeowithDataError::from)?;

    Ok(EmptyResponse)
}
