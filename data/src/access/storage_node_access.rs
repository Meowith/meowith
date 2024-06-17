use charybdis::operations::{Delete, Insert};
use charybdis::stream::CharybdisModelStream;
use scylla::{CachingSession, QueryResult};

use crate::error::MeowithDataError;
use crate::model::storage_node_model::StorageNode;

static CLUSTER_NAME: &str = "main";

pub async fn get_storage_nodes<'a>(
    session: &CachingSession,
) -> Result<CharybdisModelStream<StorageNode>, MeowithDataError> {
    StorageNode::find_by_cluster_name(String::from(CLUSTER_NAME))
        .execute(session)
        .await
        .map_err(|e| e.into())
}

pub async fn insert_storage_node(
    node: StorageNode,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    node.insert().execute(session).await.map_err(|e| e.into())
}

pub async fn remove_storage_node(
    node: StorageNode,
    session: &CachingSession,
) -> Result<QueryResult, MeowithDataError> {
    node.delete().execute(session).await.map_err(|e| e.into())
}
