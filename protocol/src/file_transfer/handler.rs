use crate::file_transfer::channel::MDSFTPChannel;
use uuid::Uuid;
use crate::file_transfer::data::LockKind;

pub trait ChannelPacketHandler: Send {
    fn handle_file_chunk(&mut self, chunk: &[u8], id: u32, is_last: bool);
    fn handle_retrieve(&mut self, chunk_id: Uuid);
    fn handle_put(&mut self, chunk_id: Uuid, content_size: u64);
    fn handle_reserve(&mut self, desired_size: u64);
    fn handle_lock_req(&mut self, chunk_id: Uuid, kind: LockKind);
}

pub trait PacketHandler: Send {
    fn channel_open(&mut self, channel: MDSFTPChannel, conn_id: Uuid);
    fn channel_close(&mut self, channel_id: u32, conn_id: Uuid);
    fn channel_err(&mut self, channel_id: u32, conn_id: Uuid);
}
