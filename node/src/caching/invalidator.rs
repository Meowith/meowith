use crate::caching::catche::NsmData;
use crate::caching::db::ValidateNonceInvalidator;
use crate::caching::node_storage_map::NodeStorageMapInvalidator;
use async_trait::async_trait;
use commons::cache::CacheId;
use std::collections::HashMap;
use std::fmt::Debug;

#[async_trait]
pub trait CacheInvalidator: Send + Debug {
    async fn invalidate(&self, cache_key: &[u8]);
}

pub fn insert_invalidator_map(
    invalidator_map: &mut HashMap<u8, Box<dyn CacheInvalidator>>,
    nsm_data: NsmData,
) {
    invalidator_map.insert(
        CacheId::ValidateNonce.into(),
        Box::new(ValidateNonceInvalidator {}),
    );
    invalidator_map.insert(
        CacheId::NodeStorageMap.into(),
        Box::new(NodeStorageMapInvalidator {
            req_ctx: nsm_data.1,
            storage_map: nsm_data.0,
        }),
    );
}
