use crate::dto::controller::UpdateStorageNodeProperties;
use crate::error::MeowithDataError;
use crate::model::microservice_node_model::{
    partial_microservice_node, MicroserviceNode, MicroserviceType, ServiceRegisterCode,
};
use charybdis::operations::{Delete, Insert, Update};
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{BigInt, Text, Timestamp, TinyInt, Uuid};
use chrono::Utc;
use scylla::transport::iterator::TypedRowIterator;
use scylla::{CachingSession, QueryResult};

static GET_ALL_NODES_QUERY: &str = "SELECT microservice_type, id, max_space, used_space, access_token, access_token_issued_at, renewal_token, address, created, register_code FROM microservice_nodes";
static GET_ALL_CODES_QUERY: &str = "SELECT code, created, valid FROM service_register_codes";

partial_microservice_node!(
    UpdateMicroservice,
    id,
    microservice_type,
    used_space,
    max_space
);

partial_microservice_node!(
    UpdateMicroseviceNodeAccessToken,
    id,
    microservice_type,
    access_token,
    access_token_issued_at
);

pub async fn get_microservices(
    session: &CachingSession,
) -> Result<TypedRowIterator<MicroserviceNode>, MeowithDataError> {
    Ok(session
        .execute_iter(GET_ALL_NODES_QUERY, &[])
        .await
        .map_err(<scylla::transport::errors::QueryError as Into<MeowithDataError>>::into)?
        .into_typed::<MicroserviceNode>())
}

pub async fn get_microservice_from_type<'a>(
    service_type: MicroserviceType,
    session: &CachingSession,
) -> Result<CharybdisModelStream<MicroserviceNode>, MeowithDataError> {
    MicroserviceNode::find_by_microservice_type(service_type.into())
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn insert_microservice_node(
    node: MicroserviceNode,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    node.insert().execute(session).await.map_err(|e| e.into())
}

pub async fn update_microservice_node(
    node: MicroserviceNode,
    session: &CachingSession,
    req: &UpdateStorageNodeProperties,
) -> Result<QueryResult, MeowithDataError> {
    let update_microservice = UpdateMicroservice {
        microservice_type: node.microservice_type,
        id: node.id,
        used_space: Some(req.used_space as i64),
        max_space: Some(req.max_space as i64),
    };
    update_microservice
        .update()
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn remove_microservice_node(
    node: MicroserviceNode,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    node.delete().execute(session).await.map_err(|e| e.into())
}

pub async fn get_service_register_code(
    code: String,
    session: &CachingSession,
) -> Result<ServiceRegisterCode, MeowithDataError> {
    ServiceRegisterCode::find_by_code(code)
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn insert_service_register_code(
    code: ServiceRegisterCode,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    code.insert().execute(session).await.map_err(|e| e.into())
}

pub async fn remove_service_register_code(
    node: ServiceRegisterCode,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    node.delete().execute(session).await.map_err(|e| e.into())
}

pub async fn update_service_register_code(
    node: ServiceRegisterCode,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    node.update().execute(session).await.map_err(|e| e.into())
}

pub async fn get_service_register_codes(
    session: &CachingSession,
) -> Result<TypedRowIterator<ServiceRegisterCode>, MeowithDataError> {
    Ok(session
        .execute_iter(GET_ALL_CODES_QUERY, &[])
        .await
        .map_err(<scylla::transport::errors::QueryError as Into<MeowithDataError>>::into)?
        .into_typed::<ServiceRegisterCode>())
}

pub async fn update_service_access_token(
    node: &MicroserviceNode,
    session: &CachingSession,
    issued_at: chrono::DateTime<Utc>,
) -> Result<QueryResult, MeowithDataError> {
    let update = UpdateMicroseviceNodeAccessToken {
        microservice_type: node.microservice_type,
        id: node.id,
        access_token_issued_at: issued_at,
        access_token: node.access_token.clone(),
    };
    update.update().execute(session).await.map_err(|e| e.into())
}
