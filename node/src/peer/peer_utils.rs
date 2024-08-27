use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use commons::context::request_context::RequestContext;
use data::dto::controller::StorageResponse;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub async fn fetch_peer_storage_info<T: AsRef<MicroserviceRequestContext>>(
    req_ctx: T,
) -> reqwest::Result<StorageResponse> {
    req_ctx
        .as_ref()
        .client()
        .await
        .get(req_ctx.as_ref().controller("/api/internal/health/storage"))
        .send()
        .await?
        .json::<StorageResponse>()
        .await
}

/// Apply new total peer data to the current context.
pub async fn apply_peers(
    req_ctx: &Arc<MicroserviceRequestContext>,
    storage_map: &NodeStorageMap,
    peers: StorageResponse,
) {
    let processed_storage_map: HashMap<Uuid, u64> = peers
        .clone()
        .peers
        .into_iter()
        .map(|elem| (elem.0, elem.1.storage))
        .collect();

    let mut nodes = req_ctx.node_addr.write().await;
    let mut storage_map = storage_map.write().await;

    nodes.clear();
    for peer in peers.peers {
        nodes.insert(peer.0, peer.1.addr.to_string());
    }
    *storage_map = processed_storage_map;
}
