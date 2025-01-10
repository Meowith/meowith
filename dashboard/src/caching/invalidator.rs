use crate::caching::db::ValidateNonceInvalidator;
use async_trait::async_trait;
use commons::cache::CacheId;
use std::collections::HashMap;
use std::fmt::Debug;

#[async_trait]
pub trait CacheInvalidator: Send + Sync + Debug {
    async fn invalidate(&self, cache_key: &[u8]);
}

pub fn insert_invalidator_map(invalidator_map: &mut HashMap<u8, Box<dyn CacheInvalidator>>) {
    invalidator_map.insert(
        CacheId::ValidateNonce.into(),
        Box::new(ValidateNonceInvalidator {}),
    );
}
