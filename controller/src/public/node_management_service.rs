use crate::public::routes::node_management::RegisterCodeCreateRequest;
use chrono::Utc;
use commons::error::std_response::NodeClientResponse;
use data::access::microservice_node_access::{
    get_service_register_code, get_service_register_codes, insert_service_register_code,
    remove_service_register_code,
};
use data::dto::entity::ServiceRegisterCodeDto;
use data::error::MeowithDataError;
use data::model::microservice_node_model::ServiceRegisterCode;
use futures_util::TryStreamExt;
use scylla::CachingSession;

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
