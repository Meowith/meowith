use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use protocol::mgpp::handler::InvalidateCacheHandler;
use crate::caching::invalidator::{insert_invalidator_map, CacheInvalidator};

//Node storage map invalidator data type
pub type NsmData = (NodeStorageMap, Arc<MicroserviceRequestContext>);

#[derive(Debug, Default)]
pub struct CacheInvalidationHandler {
    invalidators: Arc<HashMap<u8, Box<dyn CacheInvalidator>>>,
}

impl CacheInvalidationHandler {
    pub fn new(nsm_data: NsmData) -> Self {
        let mut map = HashMap::new();

        insert_invalidator_map(&mut map, nsm_data);

        CacheInvalidationHandler { invalidators: Arc::new(map) }
    }
}

#[async_trait]
impl InvalidateCacheHandler for CacheInvalidationHandler {
    async fn handle_invalidate(&self, cache_id: u32, cache_key: &[u8]) {
        let invalidator = self.invalidators.get(&(cache_id as u8));
        if let Some(invalidator) = invalidator {
            invalidator.invalidate(cache_key).await;
        }
    }
}