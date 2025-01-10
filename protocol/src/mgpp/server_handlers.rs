use crate::mgpp::handler::InvalidateCacheHandler;
use crate::mgpp::packet::MGPPPacket;
use crate::mgpp::MGPPConnection;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct MGPPServerCacheInvalidateHandler {
    pub(crate) connections: Arc<Mutex<Vec<MGPPConnection>>>,
}

#[async_trait]
impl InvalidateCacheHandler for MGPPServerCacheInvalidateHandler {
    async fn handle_invalidate(&self, cache_id: u32, cache: &[u8]) {
        let connections = self.connections.lock().await;

        for connection in &*connections {
            let writer = connection.0.obtain_writer();
            let _ = writer
                .lock()
                .await
                .write_packet(MGPPPacket::InvalidateCache {
                    cache_id,
                    cache_key: cache.to_vec(),
                })
                .await;
        }
    }
}
