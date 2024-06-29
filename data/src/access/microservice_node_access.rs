use crate::error::MeowithDataError;
use crate::model::microservice_node_model::{
    partial_microservice_node, MicroserviceNode, MicroserviceType, ServiceRegisterCode,
};
use charybdis::operations::{Delete, Insert, Update};
use charybdis::stream::CharybdisModelStream;
use charybdis::types::{BigInt, Uuid};
use scylla::transport::session::TypedRowIter;
use scylla::{CachingSession, QueryResult};

static GET_ALL_NODES_QUERY: &str = "SELECT * FROM microservice_nodes";

partial_microservice_node!(
    UpdateMicroservice,
    id,
    microservice_type,
    used_space,
    max_space
);

pub async fn get_microservices(
    session: &CachingSession,
) -> Result<TypedRowIter<MicroserviceNode>, MeowithDataError> {
    session
        .execute(GET_ALL_NODES_QUERY, &[])
        .await
        .map_err(<scylla::transport::errors::QueryError as Into<MeowithDataError>>::into)?
        .rows_typed::<MicroserviceNode>()
        .map_err(|_| MeowithDataError::NotFound)
}

pub async fn get_microservice_from_type<'a>(
    service_type: MicroserviceType,
    session: &CachingSession,
) -> Result<CharybdisModelStream<MicroserviceNode>, MeowithDataError> {
    MicroserviceNode::find_by_microservice_type(service_type)
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
    used_space: i64,
    max_space: i64,
) -> Result<QueryResult, MeowithDataError> {
    let update_microservice = UpdateMicroservice {
        microservice_type: node.microservice_type,
        id: node.id,
        used_space: Some(used_space),
        max_space: Some(max_space),
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
