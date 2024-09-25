use actix_web::web;
use commons::error::std_response::NodeClientResponse;
use data::access::user_access::{maybe_get_user_from_id, maybe_get_user_from_name};
use data::dto::entity::MaybeUserDTO;
use scylla::CachingSession;
use uuid::Uuid;

pub async fn do_get_user_by_id(
    id: Uuid,
    session: &CachingSession,
) -> NodeClientResponse<web::Json<MaybeUserDTO>> {
    let user = maybe_get_user_from_id(id, session).await?;
    Ok(web::Json(MaybeUserDTO {
        user: user.map(|x| x.into()),
    }))
}

pub async fn do_get_user_by_name(
    name: String,
    session: &CachingSession,
) -> NodeClientResponse<web::Json<MaybeUserDTO>> {
    let user = maybe_get_user_from_name(name, session).await?;
    Ok(web::Json(MaybeUserDTO {
        user: user.map(|x| x.into()),
    }))
}
