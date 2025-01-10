use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::framework::error::ProtocolResult;
use crate::framework::writer::PacketWriter;
use crate::mgpp::packet::{MGPPPacket, MGPPPacketHandler};

#[async_trait]
pub trait InvalidateCacheHandler: Send + Sync + Debug {
    async fn handle_invalidate(&self, cache_id: u32, cache_key: &[u8]);
}

#[derive(Debug)]
pub struct MGPPHandlers {
    pub invalidate_cache: Box<dyn InvalidateCacheHandler>,
}

impl MGPPHandlers {
    pub fn new(invalidate_cache: Box<dyn InvalidateCacheHandler>) -> Self {
        Self { invalidate_cache }
    }
}

#[derive(Debug)]
pub struct MGPPHandlersMapper {
    handlers: MGPPHandlers
}

unsafe impl Sync for MGPPHandlersMapper {}

impl MGPPHandlersMapper {
    pub(crate) fn new(handlers: MGPPHandlers) -> MGPPHandlersMapper {
        Self { handlers }
    }
}
#[async_trait]
impl MGPPPacketHandler<MGPPPacket> for MGPPHandlersMapper {
    async fn handle_invalidate_cache(&self, _: Arc<Mutex<PacketWriter<MGPPPacket>>>, cache_id: u32, cache_key: Vec<u8>) -> ProtocolResult<()> {
        self.handlers.invalidate_cache.handle_invalidate(cache_id, cache_key.as_slice()).await;

        Ok(())
    }
}