use crate::error::MeowithDataError;
use crate::model::app_model::{
    App, AppByOwner, AppMember, AppToken, MemberByUser, UpdateAppQuota, UserRole,
};
use charybdis::errors::CharybdisError;
use charybdis::operations::{Delete, Insert, Update};
use charybdis::stream::CharybdisModelStream;
use log::info;
use scylla::{CachingSession, QueryResult};
use std::collections::HashSet;
use uuid::Uuid;

pub type UserRoleItem = Result<UserRole, CharybdisError>;
pub type AppTokenItem = Result<AppToken, CharybdisError>;

pub async fn insert_app(
    app: &App,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    app.insert().execute(session).await.map_err(|e| e.into())
}

pub async fn delete_app(app: &App, session: &CachingSession) -> Result<(), MeowithDataError> {
    AppToken::delete_by_app_id(app.id).execute(session).await?;
    UserRole::delete_by_app_id(app.id).execute(session).await?;
    AppMember::delete_by_app_id(app.id).execute(session).await?;
    app.delete().execute(session).await?;
    Ok(())
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

pub async fn maybe_get_app_member(
    app_id: Uuid,
    user_id: Uuid,
    session: &CachingSession,
) -> Result<Option<AppMember>, MeowithDataError> {
    AppMember::maybe_find_first_by_app_id_and_member_id(app_id, user_id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn insert_app_member(
    app_id: Uuid,
    member_id: Uuid,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    AppMember {
        app_id,
        member_id,
        member_roles: Some(HashSet::new()),
    }
    .insert()
    .execute(session)
    .await
    .map_err(|e| e.into())
}

pub async fn delete_app_member(
    member: &AppMember,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    member.delete().execute(session).await.map_err(|e| e.into())
}

pub async fn update_app_member(
    member: &AppMember,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    member.update().execute(session).await.map_err(|e| e.into())
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

pub async fn get_app_role(
    app_id: Uuid,
    name: String,
    session: &CachingSession,
) -> Result<UserRole, MeowithDataError> {
    UserRole::find_first_by_app_id_and_name(app_id, name)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn insert_app_role(
    role: UserRole,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    role.insert().execute(session).await.map_err(|e| e.into())
}

pub async fn delete_app_role(
    role: UserRole,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    role.delete().execute(session).await.map_err(|e| e.into())
}

pub async fn update_app_role(
    role: UserRole,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    role.update().execute(session).await.map_err(|e| e.into())
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

pub async fn get_app_tokens(
    app_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<AppToken>, MeowithDataError> {
    AppToken::find_by_app_id(app_id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_app_tokens_by_issuer(
    app_id: Uuid,
    issuer_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<AppToken>, MeowithDataError> {
    AppToken::find_by_app_id_and_issuer_id(app_id, issuer_id)
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

pub async fn get_apps_by_owner(
    owner_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<AppByOwner>, MeowithDataError> {
    AppByOwner::find_by_owner_id(owner_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn get_members_by_id(
    user_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<MemberByUser>, MeowithDataError> {
    MemberByUser::find_by_member_id(user_id)
        .execute(session)
        .await
        .map_err(MeowithDataError::from)
}

pub async fn update_app_quota(
    id: Uuid,
    quota: i64,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    info!("{id} quota: {quota}");
    let update = UpdateAppQuota { id, quota };

    update.update().execute(session).await.map_err(|e| e.into())
}
