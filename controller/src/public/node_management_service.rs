use crate::public::routes::node_management::RegisterCodeCreateRequest;
use crate::AppState;
use actix_web::web::Data;
use chrono::Utc;
use commons::error::std_response::NodeClientResponse;
use data::access::microservice_node_access::{
    get_microservice_node, get_service_register_code, get_service_register_codes,
    insert_service_register_code, remove_microservice_node, remove_service_register_code,
};
use data::dto::entity::ServiceRegisterCodeDto;
use data::error::MeowithDataError;
use data::model::microservice_node_model::ServiceRegisterCode;
use futures_util::TryStreamExt;
use scylla::client::caching_session::CachingSession;
use uuid::Uuid;

pub async fn do_create_register_code(
    req: RegisterCodeCreateRequest,
    session: &CachingSession,
) -> NodeClientResponse<String> {
    let code = ServiceRegisterCode {
        code: req.code.clone(),
        created: Utc::now(),
        valid: true,
    };

    insert_service_register_code(code, session).await?;

    Ok(req.code)
}

pub async fn do_delete_register_code(
    code: String,
    session: &CachingSession,
) -> NodeClientResponse<()> {
    let code = get_service_register_code(code, session).await?;
    remove_service_register_code(code, session).await?;
    Ok(())
}

pub async fn do_list_register_codes(
    session: &CachingSession,
) -> NodeClientResponse<Vec<ServiceRegisterCodeDto>> {
    let codes: Vec<ServiceRegisterCode> = get_service_register_codes(session)
        .await?
        .try_collect()
        .await
        .map_err(|_| MeowithDataError::UnknownFailure)?;
    Ok(codes.into_iter().map(|x| x.into()).collect())
}

pub async fn do_delete_node(
    id: Uuid,
    node_type: i8,
    state: &Data<AppState>,
) -> NodeClientResponse<()> {
    let node = get_microservice_node(id, node_type, &state.session).await?;
    remove_microservice_node(node, &state.session).await?;
    state.req_ctx.remove_node_from_maps(id).await;

    Ok(())
}
