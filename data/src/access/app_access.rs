use crate::error::MeowithDataError;
use crate::model::app_model::{App, AppMember};
use charybdis::stream::CharybdisModelStream;
use charybdis::types::Uuid;
use scylla::CachingSession;

pub async fn get_apps_from_owner(
    owner_id: Uuid,
    session: &CachingSession,
) -> Result<CharybdisModelStream<App>, MeowithDataError> {
    App::find_by_owner_id(owner_id)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_app_by_name(
    owner_id: Uuid,
    name: String,
    session: &CachingSession,
) -> Result<App, MeowithDataError> {
    App::find_first_by_owner_id_and_name(owner_id, name)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn get_app_by_id(
    owner_id: Uuid,
    id: Uuid,
    session: &CachingSession,
) -> Result<App, MeowithDataError> {
    App::find_by_owner_id_and_id(owner_id, id)
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
