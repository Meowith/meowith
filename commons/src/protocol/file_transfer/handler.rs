use crate::protocol::file_transfer::channel::MDSFTPChannel;
use uuid::Uuid;

pub trait ChannelHandler: Send {
    fn handle_file_chunk(&mut self);
    fn handle_retrieve(&mut self);
    fn handle_put(&mut self);
    fn handle_reserve(&mut self);
    fn handle_lock_req(&mut self);
}

pub trait PacketHandler: Send {
    fn channel_open(&mut self, channel: MDSFTPChannel, conn_id: Uuid);
    fn channel_close(&mut self, channel_id: u32, conn_id: Uuid);
    fn channel_err(&mut self, channel_id: u32, conn_id: Uuid);
}
