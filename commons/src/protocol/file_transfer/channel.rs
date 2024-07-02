use std::sync::{Arc, Weak};

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::protocol::file_transfer::data::LockKind;
use crate::protocol::file_transfer::error::MDSFTPResult;
use crate::protocol::file_transfer::net::packet_reader::PacketReader;
use crate::protocol::file_transfer::net::packet_writer::PacketWriter;
use crate::protocol::file_transfer::net::wire::MDSFTPRawPacket;

pub struct MDSFTPChannel {
    pub(crate) _internal_channel: Arc<InternalMDSFTPChannel>
}

impl MDSFTPChannel {

    pub async fn request_lock(&self, kind: LockKind, chunk_id: Uuid) -> MDSFTPResult<LockKind> {
        self._internal_channel.request_lock(kind, chunk_id).await
    }

    pub async fn try_reserve(&self, desired: u64) -> MDSFTPResult<Uuid> {
        self._internal_channel.try_reserve(desired).await
    }

    pub async fn send_chunk(&self, is_last: bool, id: u32, content: &[u8]) -> MDSFTPResult<()> {
        self._internal_channel.send_chunk(is_last, id, content).await
    }

}

impl Drop for MDSFTPChannel {
    fn drop(&mut self) {
        let internal_ref = self._internal_channel.clone();
        tokio::spawn(async move {
            internal_ref.cleanup().await;
        });
    }
}

#[allow(unused)]
pub(crate) struct InternalMDSFTPChannel {
    pub(crate) id: u32,
    pub(crate) writer_ref: Weak<Mutex<PacketWriter>>,
    pub(crate) reader_ref: Weak<PacketReader>
}

impl InternalMDSFTPChannel {
    pub(crate) fn new(
        id: u32,
        writer_ref: Weak<Mutex<PacketWriter>>,
        reader_ref: Weak<PacketReader>,
    ) -> Self {
        InternalMDSFTPChannel {
            id,
            writer_ref,
            reader_ref,
        }
    }

    /// Unregister the listener
    pub(super) async fn cleanup(&self) {
        let reader = self.reader_ref.upgrade();
        if reader.is_some() {
            reader.unwrap().remove_channel(self.id).await;
        }
    }

    pub(super) async fn request_lock(&self, kind: LockKind, chunk_id: Uuid) -> MDSFTPResult<LockKind> {
        todo!()
    }

    pub(super) async fn try_reserve(&self, desired: u64) -> MDSFTPResult<Uuid> {
        todo!()
    }

    pub(super) async fn send_chunk(&self, is_last: bool, id: u32, content: &[u8]) -> MDSFTPResult<()> {
        todo!()
    }

    pub(crate) async fn handle_packet(&self, packet: MDSFTPRawPacket) {
    }

    pub(crate) async fn interrupt(&self) {
    }

}