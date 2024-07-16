use crate::file_transfer::channel_handler::MeowithMDSFTPChannelPacketHandler;
use crate::io::fragment_ledger::FragmentLedger;
use async_trait::async_trait;
use log::debug;
use protocol::file_transfer::channel::MDSFTPChannel;
use protocol::file_transfer::handler::PacketHandler;
use uuid::Uuid;

pub const BUFFER_SIZE: u16 = 10;

pub struct MeowithMDSFTPPacketHandler {
    fragment_ledger: FragmentLedger,
    fragment_size: u32,
}

impl MeowithMDSFTPPacketHandler {
    pub(crate) fn new(fragment_ledger: FragmentLedger, fragment_size: u32) -> Self {
        MeowithMDSFTPPacketHandler {
            fragment_ledger,
            fragment_size,
        }
    }
}

#[async_trait]
impl PacketHandler for MeowithMDSFTPPacketHandler {
    async fn channel_incoming(&mut self, channel: MDSFTPChannel, conn_id: Uuid) {
        debug!("Channel open {conn_id}");
        let await_handler = channel
            .set_incoming_handler(Box::new(MeowithMDSFTPChannelPacketHandler::new(
                self.fragment_ledger.clone(),
                BUFFER_SIZE,
                self.fragment_size,
            )))
            .await;
        tokio::spawn(async move {
            let _no_drop = channel;
            await_handler.await;
        });
    }

    async fn channel_close(&mut self, _channel_id: u32, _conn_id: Uuid) {
        debug!("Channel close");
    }

    async fn channel_err(&mut self, _channel_id: u32, _conn_id: Uuid) {
        debug!("Channel err")
    }
}
