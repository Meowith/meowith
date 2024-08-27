use crate::error::node::NodeError;
use data::access::microservice_node_access::update_microservice_node;
use data::dto::controller::UpdateStorageNodeProperties;
use data::model::microservice_node_model::MicroserviceNode;
use scylla::CachingSession;

pub async fn perform_storage_node_properties_update(
    req: UpdateStorageNodeProperties,
    session: &CachingSession,
    node: MicroserviceNode,
) -> Result<(), NodeError> {
    update_microservice_node(node, session, req.used_space as i64, req.max_space as i64)
        .await
        .map_err(|_| NodeError::InternalError)?;
    Ok(())
}
