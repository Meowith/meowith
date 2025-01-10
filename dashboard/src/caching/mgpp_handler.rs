use std::collections::HashMap;
use async_trait::async_trait;
use protocol::mgpp::handler::InvalidateCacheHandler;
use crate::caching::invalidator::{insert_invalidator_map, CacheInvalidator};

#[derive(Debug, Default)]
pub struct CacheInvalidationHandler {
    invalidators: HashMap<u8, Box<dyn CacheInvalidator>>,
}

impl CacheInvalidationHandler {
    pub fn new() -> Self {
        let mut map = HashMap::new();

        insert_invalidator_map(&mut map);

        CacheInvalidationHandler { invalidators: map }
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
