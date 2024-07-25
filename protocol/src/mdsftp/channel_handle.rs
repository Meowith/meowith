use crate::mdsftp::channel::{InternalMDSFTPChannel, MDSFTPChannel};
use crate::mdsftp::data::{ChunkErrorKind, LockKind};
use crate::mdsftp::error::MDSFTPResult;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;

pub struct ChannelAwaitHandle {
    pub(crate) _receiver: Receiver<MDSFTPResult<()>>,
}

impl Future for ChannelAwaitHandle {
    type Output = Option<MDSFTPResult<()>>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        self._receiver.poll_recv(ctx)
    }
}

#[derive(Clone)]
pub struct MDSFTPHandlerChannel {
    pub(crate) _internal_channel: Weak<InternalMDSFTPChannel>,
}

macro_rules! define_respond_method {
    ($name:ident($($param:ident: $ptype:ty),*) -> $ret:ty { $channel_method:ident($($arg:expr),*) }) => {
        pub async fn $name(&self, $($param: $ptype),*) -> $ret {
            let channel = self
                ._internal_channel
                .upgrade()
                .expect("Attempted to use a dead channel");
            let x = channel.$channel_method($($arg),*).await; x
        }
    };
}

impl MDSFTPHandlerChannel {
    pub(crate) fn new(channel: &MDSFTPChannel) -> Self {
        MDSFTPHandlerChannel {
            _internal_channel: Arc::downgrade(&channel._internal_channel),
        }
    }

    define_respond_method!(respond_chunk(is_last: bool, id: u32, content: &[u8]) -> MDSFTPResult<()> {
        send_chunk(is_last, id, content)
    });

    define_respond_method!(respond_lock_ok(chunk_id: Uuid, kind: LockKind) -> MDSFTPResult<()> {
        respond_lock_ok(chunk_id, kind)
    });

    define_respond_method!(respond_lock_err(chunk_id: Uuid, kind: LockKind, error_kind: ChunkErrorKind) -> MDSFTPResult<()> {
        respond_lock_err(chunk_id, kind, error_kind)
    });

    define_respond_method!(respond_reserve_ok(chunk_id: Uuid, chunk_buffer: u16) -> MDSFTPResult<()> {
        respond_reserve_ok(chunk_id, chunk_buffer)
    });

    define_respond_method!(respond_reserve_err(available_space: u64) -> MDSFTPResult<()> {
        respond_reserve_err(available_space)
    });

    define_respond_method!(respond_receive_ack(chunk_id: u32) -> MDSFTPResult<()> {
        respond_receive_ack(chunk_id)
    });

    define_respond_method!(close(result: MDSFTPResult<()>) -> () {
        mark_handler_closed(result)
    });
}
