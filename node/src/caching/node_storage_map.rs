use crate::caching::invalidator::CacheInvalidator;
use crate::peer::peer_utils::{apply_peers, fetch_peer_storage_info};
use async_trait::async_trait;
use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use log::warn;
use std::sync::Arc;

#[derive(Debug)]
pub struct NodeStorageMapInvalidator {
    pub req_ctx: Arc<MicroserviceRequestContext>,
    pub storage_map: NodeStorageMap,
}

#[async_trait]
impl CacheInvalidator for NodeStorageMapInvalidator {
    async fn invalidate(&self, _cache_key: &[u8]) {
        let new_nodes = fetch_peer_storage_info(&self.req_ctx).await;
        match new_nodes {
            Ok(peers) => {
                apply_peers(&self.req_ctx, &self.storage_map, peers).await;
            }
            Err(e) => {
                warn!("Failed to refresh the node storage map {e}");
            }
        }
    }
}
