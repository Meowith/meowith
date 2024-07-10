use std::sync::Arc;
use crate::io::fragment_ledger::FragmentLedger;
use crate::locking::file_read_guard::FileReadGuard;
use crate::locking::file_write_guard::FileWriteGuard;
use async_trait::async_trait;
use protocol::file_transfer::data::{ChunkErrorKind, LockKind};
use protocol::file_transfer::error::MDSFTPResult;
use protocol::file_transfer::handler::{Channel, ChannelPacketHandler};
use uuid::Uuid;

pub struct MeowithMDSFTPChannelPacketHandler {
    fragment_ledger: FragmentLedger,
    read_guard: Option<Arc<FileReadGuard<Uuid>>>,
    write_guard: Option<Arc<FileWriteGuard<Uuid>>>,
}

impl MeowithMDSFTPChannelPacketHandler {
    pub fn new(fragment_ledger: FragmentLedger) -> Self {
        MeowithMDSFTPChannelPacketHandler {
            fragment_ledger,
            read_guard: None,
            write_guard: None,
        }
    }
}

#[allow(unused)]
#[async_trait]
impl ChannelPacketHandler for MeowithMDSFTPChannelPacketHandler {
    async fn handle_file_chunk(
        &mut self,
        channel: Channel,
        chunk: &[u8],
        id: u32,
        is_last: bool,
    ) -> MDSFTPResult<()> {
        todo!()
    }

    async fn handle_retrieve(&mut self, channel: Channel, chunk_id: Uuid) -> MDSFTPResult<()> {
        todo!()
    }

    async fn handle_put(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        content_size: u64,
    ) -> MDSFTPResult<()> {
        todo!()
    }

    async fn handle_reserve(&mut self, channel: Channel, desired_size: u64) -> MDSFTPResult<()> {
        todo!()
    }

    async fn handle_lock_req(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        kind: LockKind,
    ) -> MDSFTPResult<()> {
        if !self.fragment_ledger.fragment_exists(&chunk_id) {
            channel.respond_lock_err(chunk_id, kind, ChunkErrorKind::NotFound);
            channel.close();
            return Ok(());
        }

        match kind {
            LockKind::Read => match self.fragment_ledger.lock_table().try_read(chunk_id).await {
                Ok(guard) => {
                    self.read_guard = Some(Arc::new(guard));
                }
                Err(_) => {
                    channel.respond_lock_err(chunk_id, kind, ChunkErrorKind::NotAvailable);
                }
            },
            LockKind::Write => match self.fragment_ledger.lock_table().try_write(chunk_id).await {
                Ok(guard) => {
                    self.write_guard = Some(Arc::new(guard));
                }
                Err(_) => {
                    channel.respond_lock_err(chunk_id, kind, ChunkErrorKind::NotAvailable);
                }
            },
            _ => unreachable!(),
        };

        return Ok(());
    }
}
