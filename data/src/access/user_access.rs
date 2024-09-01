use crate::error::MeowithDataError;
use crate::model::user_model::{User, UsersByName};
use charybdis::operations::{Find, Insert};
use scylla::{CachingSession, QueryResult};
use uuid::Uuid;

pub async fn get_user_from_name(
    name: String,
    session: &CachingSession,
) -> Result<UsersByName, MeowithDataError> {
    UsersByName::find_first_by_name(name)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_user_from_id(
    id: Uuid,
    session: &CachingSession,
) -> Result<User, MeowithDataError> {
    User::find_first_by_id(id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn maybe_get_user_from_id(
    id: Uuid,
    session: &CachingSession,
) -> Result<Option<User>, MeowithDataError> {
    User::maybe_find_first_by_id(id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn maybe_get_first_user(
    session: &CachingSession,
) -> Result<Option<User>, MeowithDataError> {
    User::maybe_find_first("select id, session_id, name, auth_identifier, global_role, created, last_modified from users", ())
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn insert_user(
    user: &User,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    user.insert().execute(session).await.map_err(|e| e.into())
}
