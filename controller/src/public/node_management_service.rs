use crate::public::routes::node_management::RegisterCodeCreateRequest;
use chrono::Utc;
use commons::error::std_response::NodeClientResponse;
use data::access::microservice_node_access::insert_service_register_code;
use data::model::microservice_node_model::ServiceRegisterCode;
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
