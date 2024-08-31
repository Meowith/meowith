use crate::public::routes::application::CreateApplicationRequest;
use actix_web::web;
use chrono::Utc;
use commons::error::std_response::{NodeClientError, NodeClientResponse};
use data::access::app_access::{
    delete_app, delete_app_member, get_app_by_id, insert_app, insert_app_member,
    maybe_get_app_member,
};
use data::access::file_access::maybe_get_first_bucket;
use data::access::user_access::maybe_get_user_from_id;
use data::dto::config::GeneralConfiguration;
use data::dto::entity::AppDto;
use data::model::app_model::App;
use data::model::user_model::User;
use scylla::CachingSession;
use uuid::Uuid;

pub async fn do_create_app(
    req: CreateApplicationRequest,
    session: &CachingSession,
    user: User,
    global_config: &GeneralConfiguration,
) -> NodeClientResponse<web::Json<AppDto>> {
    let now = Utc::now();
    let app = App {
        id: Uuid::new_v4(),
        name: req.name,
        owner_id: user.id,
        quota: global_config.default_application_quota as i64,
        created: now,
        last_modified: now,
    };

    insert_app(&app, session).await?;

    Ok(web::Json(app.into()))
}

pub async fn do_delete_app(
    id: Uuid,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(id, session).await?;
    if user.id != app.owner_id {
        return Err(NodeClientError::BadAuth);
    }

    let bucket = maybe_get_first_bucket(id, session).await?;
    if bucket.is_some() {
        return Err(NodeClientError::EntityExists);
    }

    delete_app(&app, session).await?;
    Ok(())
}

pub async fn do_add_member(
    member_id: Uuid,
    app_id: Uuid,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(app_id, session).await?;
    if user.id != app.owner_id {
        return Err(NodeClientError::BadAuth);
    }
    let user = maybe_get_user_from_id(member_id, session).await?;
    if user.is_some() {
        insert_app_member(app.id, member_id, session).await?;
        Ok(())
    } else {
        Err(NodeClientError::NotFound)
    }
}

pub async fn do_delete_member(
    member_id: Uuid,
    app_id: Uuid,
    session: &CachingSession,
    user: User,
) -> NodeClientResponse<()> {
    let app = get_app_by_id(app_id, session).await?;
    if user.id != app.owner_id {
        return Err(NodeClientError::BadAuth);
    }
    let member = maybe_get_app_member(app.id, member_id, session).await?;
    if let Some(member) = member {
        delete_app_member(&member, session).await?;
        Ok(())
    } else {
        Err(NodeClientError::NotFound)
    }
}
