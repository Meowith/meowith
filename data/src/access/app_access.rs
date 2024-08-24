use crate::error::MeowithDataError;
use crate::model::app_model::{App, AppByOwner, AppMember, AppToken, UserRole};
use charybdis::errors::CharybdisError;
use charybdis::operations::{Delete, Insert};
use charybdis::stream::CharybdisModelStream;
use scylla::{CachingSession, QueryResult};
use uuid::Uuid;

pub type UserRoleItem = Result<UserRole, CharybdisError>;

pub async fn insert_app(
    app: &App,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    app.insert().execute(session).await.map_err(|e| e.into())
}

pub async fn get_app_member(
    app_id: Uuid,
    user_id: Uuid,
    session: &CachingSession,
) -> Result<AppMember, MeowithDataError> {
    AppMember::find_by_app_id_and_member_id(app_id, user_id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_app_roles(
    app_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<UserRole>, MeowithDataError> {
    UserRole::find_by_app_id(app_id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_apps_from_owner(
    owner_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<AppByOwner>, MeowithDataError> {
    AppByOwner::find_by_owner_id(owner_id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_app_by_id(id: Uuid, session: &CachingSession) -> Result<App, MeowithDataError> {
    App::find_by_id(id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_app_members(
    app_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<AppMember>, MeowithDataError> {
    AppMember::find_by_app_id(app_id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_app_token(
    app_id: Uuid,
    issuer_id: Uuid,
    name: String,
    session: &CachingSession,
) -> Result<AppToken, MeowithDataError> {
    AppToken::find_by_app_id_and_issuer_id_and_name(app_id, issuer_id, name)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn insert_app_token(
    token: &AppToken,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    token
        .insert()
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn delete_app_token(
    token: &AppToken,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    token
        .delete()
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}
