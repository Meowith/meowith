use async_trait::async_trait;
use tokio::io::{AsyncRead, BufReader, BufWriter};
use uuid::Uuid;

use crate::file_transfer::channel::MDSFTPChannel;
use crate::file_transfer::channel_handle::MDSFTPHandlerChannel;
use crate::file_transfer::data::LockKind;
use crate::file_transfer::error::MDSFTPResult;

pub type Channel = MDSFTPHandlerChannel;

#[async_trait]
pub trait ChannelPacketHandler: Send {
    async fn handle_file_chunk(
        &mut self,
        channel: Channel,
        chunk: &[u8],
        id: u32,
        is_last: bool,
    ) -> MDSFTPResult<()>;

    async fn handle_retrieve(&mut self, channel: Channel, chunk_id: Uuid) -> MDSFTPResult<()>;

    async fn handle_put(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        content_size: u64,
    ) -> MDSFTPResult<()>;

    async fn handle_reserve(
        &mut self,
        channel: Channel,
        desired_size: u64,
        auto_start: bool,
    ) -> MDSFTPResult<()>;

    async fn handle_lock_req(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        kind: LockKind,
    ) -> MDSFTPResult<()>;

    async fn handle_receive_ack(&mut self, channel: Channel, chunk_id: u32) -> MDSFTPResult<()>;

    async fn handle_interrupt(&self) -> MDSFTPResult<()>;
}

#[async_trait]
pub trait UploadDelegator<T: AsyncRead + Unpin>: Send {
    async fn delegate_upload(
        &mut self,
        channel: Channel,
        source: BufReader<T>,
        size: u64,
        chunk_buffer: u16,
    ) -> MDSFTPResult<()>;
}

#[async_trait]
pub trait DownloadDelegator<T: AsyncRead + Unpin>: Send {
    async fn delegate_download(output: BufWriter<T>);
}

#[async_trait]
pub trait PacketHandler: Send {
    /// Called when a remote channel is closed
    async fn channel_incoming(&mut self, channel: MDSFTPChannel, conn_id: Uuid);
    async fn channel_close(&mut self, channel_id: u32, conn_id: Uuid);
    async fn channel_err(&mut self, channel_id: u32, conn_id: Uuid);
}
