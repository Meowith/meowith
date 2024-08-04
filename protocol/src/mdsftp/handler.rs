use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mdsftp::channel::MDSFTPChannel;
use crate::mdsftp::channel_handle::MDSFTPHandlerChannel;
use crate::mdsftp::data::{CommitFlags, LockKind, PutFlags, ReserveFlags};
use crate::mdsftp::error::MDSFTPResult;

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

    async fn handle_retrieve(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        chunk_buffer: u16,
    ) -> MDSFTPResult<()>;

    async fn handle_put(
        &mut self,
        channel: Channel,
        flags: PutFlags,
        chunk_id: Uuid,
        content_size: u64,
    ) -> MDSFTPResult<()>;

    async fn handle_reserve(
        &mut self,
        channel: Channel,
        desired_size: u64,
        flags: ReserveFlags,
    ) -> MDSFTPResult<()>;

    async fn handle_lock_req(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        kind: LockKind,
    ) -> MDSFTPResult<()>;

    async fn handle_receive_ack(&mut self, channel: Channel, chunk_id: u32) -> MDSFTPResult<()>;

    async fn handle_reserve_cancel(&mut self, channel: Channel, chunk_id: Uuid)
        -> MDSFTPResult<()>;

    async fn handle_delete_chunk(&mut self, channel: Channel, chunk_id: Uuid) -> MDSFTPResult<()>;

    async fn handle_commit(
        &mut self,
        channel: Channel,
        chunk_id: Uuid,
        flags: CommitFlags,
    ) -> MDSFTPResult<()>;

    async fn handle_interrupt(&mut self) -> MDSFTPResult<()>;
}

#[async_trait]
pub trait UploadDelegator: Send {
    async fn delegate_upload(
        &mut self,
        channel: Channel,
        source: AbstractReadStream,
        size: u64,
        chunk_buffer: u16,
    ) -> MDSFTPResult<()>;
}

#[async_trait]
pub trait DownloadDelegator: Send {
    async fn delegate_download(
        &mut self,
        channel: Channel,
        output: AbstractWriteStream,
        auto_close: bool,
    ) -> MDSFTPResult<()>;
}

#[async_trait]
pub trait PacketHandler: Send {
    /// Called when a remote channel is closed
    async fn channel_incoming(&mut self, channel: MDSFTPChannel, conn_id: Uuid);
    async fn channel_close(&mut self, channel_id: u32, conn_id: Uuid);
    async fn channel_err(&mut self, channel_id: u32, conn_id: Uuid);
}

pub type AbstractReader = Pin<Box<dyn AsyncRead + Unpin + Send>>;
pub type AbstractWriter = Pin<Box<dyn AsyncWrite + Unpin + Send>>;
pub type AbstractReadStream = Arc<Mutex<AbstractReader>>;
pub type AbstractWriteStream = Arc<Mutex<AbstractWriter>>;
