use scylla::CachingSession;
use data::access::storage_node_access::get_storage_nodes;
use crate::discovery::routes::NodeRegisterRequest;
use crate::error::node::NodeError;

pub async fn perform_register_node(req: NodeRegisterRequest, session: &CachingSession) -> Result<(), NodeError> {
    let storage_node_stream = get_storage_nodes(session)
        .await
        .map_err(|_| NodeError::InternalError)?;

    for node in storage_node_stream.try_collect()
        .await
        .map_err(|_| NodeError::InternalError)? {

    };

    Ok(())
}