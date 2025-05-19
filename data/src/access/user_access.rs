use crate::error::MeowithDataError;
use crate::model::permission_model::GlobalRole;
use crate::model::user_model::{
    UpdateUser, UpdateUserQuota, UpdateUserRole, User, UsersByAuth, UsersByName,
};
use charybdis::operations::{Find, Insert, Update};
use scylla::client::caching_session::CachingSession;
use scylla::client::pager::TypedRowStream;
use scylla::errors::PagerExecutionError;
use scylla::response::query_result::QueryResult;
use uuid::Uuid;

static GET_ALL_USERS_QUERY: &str =
    "SELECT id, session_id, name, auth_identifier, quota, global_role, created, last_modified FROM users";

pub async fn get_user_from_name(
    name: String,
    session: &CachingSession,
) -> Result<UsersByName, MeowithDataError> {
    UsersByName::find_first_by_name(name)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_user_from_auth(
    auth_identifier: String,
    session: &CachingSession,
) -> Result<UsersByAuth, MeowithDataError> {
    UsersByAuth::find_first_by_auth_identifier(auth_identifier)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn update_user(
    id: Uuid,
    name: String,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    let update = UpdateUser { id, name };

    update.update().execute(session).await.map_err(|e| e.into())
}

pub async fn update_user_role(
    id: Uuid,
    role: GlobalRole,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    let update = UpdateUserRole {
        id,
        global_role: role.into(),
    };

    update.update().execute(session).await.map_err(|e| e.into())
}

pub async fn update_user_quota(
    id: Uuid,
    quota: u64,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    let update = UpdateUserQuota {
        id,
        quota: quota as i64,
    };

    update.update().execute(session).await.map_err(|e| e.into())
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

pub async fn maybe_get_user_from_name(
    id: String,
    session: &CachingSession,
) -> Result<Option<UsersByName>, MeowithDataError> {
    UsersByName::maybe_find_first_by_name(id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn maybe_get_first_user(
    session: &CachingSession,
) -> Result<Option<User>, MeowithDataError> {
    User::maybe_find_first("select id, session_id, name, auth_identifier, quota, global_role, created, last_modified from users", ())
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

pub async fn get_all_users(
    session: &CachingSession,
) -> Result<TypedRowStream<User>, MeowithDataError> {
    session
        .execute_iter(GET_ALL_USERS_QUERY, &[])
        .await
        .map_err(<PagerExecutionError as Into<MeowithDataError>>::into)?
        .rows_stream()
        .map_err(<scylla::deserialize::TypeCheckError as Into<MeowithDataError>>::into)
}
