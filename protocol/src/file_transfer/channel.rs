use std::io::Write;
use std::sync::{Arc, Weak};

use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use uuid::{Bytes, Uuid};

use crate::file_transfer::channel_handle::{ChannelAwaitHandle, MDSFTPHandlerChannel};
use crate::file_transfer::data::{
    ChunkErrorKind, LockAcquireResult, LockKind, ReserveFlags, ReserveResult,
};
use crate::file_transfer::error::{MDSFTPError, MDSFTPResult};
use crate::file_transfer::handler::{ChannelPacketHandler, DownloadDelegator, UploadDelegator};
use crate::file_transfer::net::packet_reader::PacketReader;
use crate::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::file_transfer::net::packet_writer::PacketWriter;
use crate::file_transfer::net::wire::MDSFTPRawPacket;

pub struct MDSFTPChannel {
    pub(crate) _internal_channel: Arc<InternalMDSFTPChannel>,
}

impl MDSFTPChannel {
    pub async fn request_lock(
        &self,
        kind: LockKind,
        chunk_id: Uuid,
    ) -> MDSFTPResult<LockAcquireResult> {
        self._internal_channel.request_lock(kind, chunk_id).await
    }

    pub async fn try_reserve(
        &self,
        desired: u64,
        flags: ReserveFlags,
    ) -> MDSFTPResult<ReserveResult> {
        self._internal_channel.try_reserve(desired, flags).await
    }

    pub async fn send_chunk(&self, is_last: bool, id: u32, content: &[u8]) -> MDSFTPResult<()> {
        self._internal_channel
            .send_chunk(is_last, id, content)
            .await
    }

    pub async fn send_content<T: AsyncRead + Unpin>(
        &self,
        reader: T,
        size: u64,
        chunk_buffer: u16,
        handler: Box<impl ChannelPacketHandler + UploadDelegator<T> + 'static>,
    ) -> MDSFTPResult<ChannelAwaitHandle> {
        let channel = &self._internal_channel;
        *channel.mdsftp_handler_channel.lock().await = Some(MDSFTPHandlerChannel::new(self));
        self._internal_channel
            .send_content(handler, reader, size, chunk_buffer)
            .await
    }

    pub async fn retrieve_req(&self, chunk_id: Uuid, chunk_buffer: u16) -> MDSFTPResult<()> {
        self._internal_channel
            .retrieve_req(chunk_id, chunk_buffer)
            .await
    }

    pub async fn retrieve_content<T: AsyncWrite + Unpin>(
        &self,
        writer: T,
        handler: Box<impl ChannelPacketHandler + DownloadDelegator<T> + 'static>,
    ) -> MDSFTPResult<ChannelAwaitHandle> {
        let channel = &self._internal_channel;
        *channel.mdsftp_handler_channel.lock().await = Some(MDSFTPHandlerChannel::new(self));
        self._internal_channel
            .retrieve_content(handler, writer)
            .await
    }

    pub async fn set_incoming_handler(
        &self,
        handler: Box<dyn ChannelPacketHandler>,
    ) -> ChannelAwaitHandle {
        let channel = &self._internal_channel;
        let (tx, rx) = mpsc::channel(1);
        *channel.handler_sender.lock().await = Some(tx);
        *channel.mdsftp_handler_channel.lock().await = Some(MDSFTPHandlerChannel::new(self));
        *channel.incoming_handler.lock().await = Some(handler);
        ChannelAwaitHandle { _receiver: rx }
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

macro_rules! internal_sender_method {
    ($buffer:ident $self:ident $passed:ident $name:ident($packet_type:expr, $($param:ident: $ptype:ty),*) -> $ret:ty { $channel_method:block $finish:block }) => {

        #[allow(unused)]
        pub(crate) async fn $name(&self, $($param: $ptype),*) -> $ret {
            let writer = self
                .writer_ref
                .upgrade()
                .expect("Attempted to use a dead connection");
            let mut writer = writer.lock().await;
            let mut $buffer: Vec<u8> = Vec::with_capacity($packet_type.payload_size() as usize);
            let $self = self;

            let mut $passed = $channel_method;

            writer
            .write_raw_packet(MDSFTPRawPacket
            { packet_type: $packet_type, stream_id: $self.id, payload: $buffer, })
            .await.map_err(|_| MDSFTPError::ConnectionError)?;

            return $finish;
        }
    };
}

pub(crate) struct InternalMDSFTPChannel {
    pub(crate) id: u32,
    pub(crate) writer_ref: Weak<Mutex<PacketWriter>>,
    pub(crate) reader_ref: Weak<PacketReader>,

    pub(crate) incoming_handler: Mutex<Option<Box<dyn ChannelPacketHandler>>>,
    pub(crate) mdsftp_handler_channel: Mutex<Option<MDSFTPHandlerChannel>>,
    pub(crate) handler_sender: Mutex<Option<Sender<MDSFTPResult<()>>>>,

    lock_sender: Mutex<Option<Sender<MDSFTPResult<LockAcquireResult>>>>,
    reserve_sender: Mutex<Option<Sender<MDSFTPResult<ReserveResult>>>>,
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
            lock_sender: Mutex::new(None),
            reserve_sender: Mutex::new(None),
            incoming_handler: Mutex::new(None),
            mdsftp_handler_channel: Mutex::new(None),
            handler_sender: Mutex::new(None),
        }
    }

    /// Unregister the listener
    pub(super) async fn cleanup(&self) {
        let reader = self.reader_ref.upgrade();
        let writer = self.writer_ref.upgrade();
        if reader.is_some() {
            reader.unwrap().remove_channel(self.id).await;
        }
        if writer.is_some() {
            let _ = writer
                .unwrap()
                .lock()
                .await
                .write_raw_packet(MDSFTPRawPacket {
                    packet_type: MDSFTPPacketType::ChannelClose,
                    stream_id: self.id,
                    payload: vec![],
                })
                .await;
        }
    }

    internal_sender_method!(payload_buffer this lock request_lock(MDSFTPPacketType::LockReq, kind: LockKind, chunk_id: Uuid) -> MDSFTPResult<LockAcquireResult> {
        {
            payload_buffer.push(kind.into());
            let _ = payload_buffer.write(chunk_id.as_bytes().as_slice());
            let (tx, rx) = mpsc::channel(1);
            *this.lock_sender.lock().await = Some(tx);
            rx
        }
        { lock.recv().await.ok_or(MDSFTPError::Interrupted)? }
    });

    internal_sender_method!(payload_buffer this lock try_reserve(MDSFTPPacketType::Reserve, desired: u64, flags: ReserveFlags) -> MDSFTPResult<ReserveResult> {
        {
            payload_buffer.push(flags.into());
            let _ = payload_buffer.write(&desired.to_be_bytes());
            let (tx, rx) = mpsc::channel(1);
            *this.reserve_sender.lock().await = Some(tx);
            rx
        }
        { lock.recv().await.ok_or(MDSFTPError::Interrupted)? }
    });

    internal_sender_method!(payload_buffer this none respond_lock_ok(MDSFTPPacketType::LockAcquire, chunk_id: Uuid, kind: LockKind) -> MDSFTPResult<()> {
        {
            let kind: u8 = kind.into();
            payload_buffer.push(kind);
            let _ = payload_buffer.write(chunk_id.as_bytes().as_slice());
        }
        { Ok(()) }
    });

    internal_sender_method!(payload_buffer this none respond_lock_err(MDSFTPPacketType::LockErr, chunk_id: Uuid, kind: LockKind, error_kind: ChunkErrorKind) -> MDSFTPResult<()> {
        {
            let kind: u8 = kind.into();
            let error_kind: u8 = error_kind.into();
            payload_buffer.push(kind | error_kind);
            let _ = payload_buffer.write(chunk_id.as_bytes().as_slice());
        }
        { Ok(()) }
    });

    internal_sender_method!(payload_buffer this none respond_reserve_ok(MDSFTPPacketType::ReserveOk, chunk_id: Uuid, chunk_buffer: u16) -> MDSFTPResult<()> {
        {
            let _ = payload_buffer.write(chunk_id.as_bytes().as_slice());
            let _ = payload_buffer.write(&chunk_buffer.to_be_bytes());
        }
        { Ok(()) }
    });

    internal_sender_method!(payload_buffer this none respond_reserve_err(MDSFTPPacketType::ReserveErr, available_space: u64) -> MDSFTPResult<()> {
        { let _ = payload_buffer.write(&available_space.to_be_bytes()); }
        { Ok(()) }
    });

    internal_sender_method!(payload_buffer this none respond_receive_ack(MDSFTPPacketType::RecvAck, chunk_id: u32) -> MDSFTPResult<()> {
        { let _ = payload_buffer.write(&chunk_id.to_be_bytes()); }
        { Ok(()) }
    });

    internal_sender_method!(payload_buffer this none retrieve_req(MDSFTPPacketType::Retrieve, chunk_id: Uuid, chunk_buffer: u16) -> MDSFTPResult<()> {
        {
            let _ = payload_buffer.write(chunk_id.as_bytes().as_slice());
            let _ = payload_buffer.write(&chunk_buffer.to_be_bytes());
        }
        { Ok(()) }
    });

    pub(super) async fn send_chunk(
        &self,
        is_last: bool,
        id: u32,
        content: &[u8],
    ) -> MDSFTPResult<()> {
        let writer = self
            .writer_ref
            .upgrade()
            .expect("Attempted to use a dead connection");
        let mut writer = writer.lock().await;
        let mut header_buffer = [0u8; 5];

        header_buffer[0] = if is_last { 0x01 } else { 0x00 };
        header_buffer[1..5].copy_from_slice(id.to_be_bytes().as_slice());

        writer
            .write_chunk(self.id, &header_buffer, content)
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;

        Ok(())
    }

    pub(crate) async fn send_content<T: AsyncRead + Unpin>(
        &self,
        mut handler: Box<impl ChannelPacketHandler + UploadDelegator<T> + 'static>,
        reader: T,
        size: u64,
        chunk_buffer: u16,
    ) -> MDSFTPResult<ChannelAwaitHandle> {
        let (tx, rx) = mpsc::channel(1);
        *self.handler_sender.lock().await = Some(tx);

        let handler_channel = self
            .mdsftp_handler_channel
            .lock()
            .await
            .as_ref()
            .unwrap()
            .clone();

        handler
            .delegate_upload(handler_channel, reader, size, chunk_buffer)
            .await?;

        *self.incoming_handler.lock().await = Some(handler);

        Ok(ChannelAwaitHandle { _receiver: rx })
    }

    pub(crate) async fn retrieve_content<T: AsyncWrite + Unpin>(
        &self,
        mut handler: Box<impl ChannelPacketHandler + DownloadDelegator<T> + 'static>,
        writer: T,
    ) -> MDSFTPResult<ChannelAwaitHandle> {
        let (tx, rx) = mpsc::channel(1);
        *self.handler_sender.lock().await = Some(tx);

        let handler_channel = self
            .mdsftp_handler_channel
            .lock()
            .await
            .as_ref()
            .unwrap()
            .clone();

        handler.delegate_download(handler_channel, writer).await?;

        *self.incoming_handler.lock().await = Some(handler);

        Ok(ChannelAwaitHandle { _receiver: rx })
    }

    pub(crate) async fn handle_packet(&self, packet: MDSFTPRawPacket) -> MDSFTPResult<()> {
        // Note: the packet's payload length is pre-validated by the packet reader.
        match packet.packet_type {
            MDSFTPPacketType::ReserveOk => {
                if let Some(tx) = self.reserve_sender.lock().await.as_ref() {
                    let chunk_id = Uuid::from_bytes(
                        Bytes::try_from(&packet.payload.as_slice()[0..16])
                            .map_err(MDSFTPError::from)?,
                    );
                    let chunk_buffer =
                        u16::from_be_bytes(packet.payload[16..18].try_into().unwrap());
                    tx.send(Ok(ReserveResult {
                        chunk_id,
                        chunk_buffer,
                    }))
                    .await
                    .unwrap()
                } else {
                    debug!("Received a ReserveOk whilst not awaiting a reservation");
                }
                *self.reserve_sender.lock().await = None;
                return Ok(());
            }
            MDSFTPPacketType::ReserveErr => {
                if let Some(tx) = self.reserve_sender.lock().await.as_ref() {
                    let mut payload_bytes = [0; 8];
                    payload_bytes.copy_from_slice(packet.payload.as_slice());

                    let max_space: u64 = u64::from_be_bytes(payload_bytes);
                    tx.send(Err(MDSFTPError::ReserveError(max_space)))
                        .await
                        .unwrap()
                } else {
                    debug!("Received a ReserveErr whilst not awaiting a reservation");
                }
                *self.reserve_sender.lock().await = None;
                return Ok(());
            }
            MDSFTPPacketType::LockAcquire => {
                if let Some(tx) = self.lock_sender.lock().await.as_ref() {
                    let chunk_id = Uuid::from_bytes(
                        Bytes::try_from(&packet.payload.as_slice()[1..17])
                            .map_err(MDSFTPError::from)?,
                    );
                    tx.send(Ok(LockAcquireResult {
                        kind: packet.payload[0].into(),
                        chunk_id,
                    }))
                    .await
                    .unwrap();
                } else {
                    debug!("Received a LockAcquire whilst not awaiting a lock");
                }
                *self.lock_sender.lock().await = None;
                return Ok(());
            }
            MDSFTPPacketType::LockErr => {
                if let Some(tx) = self.lock_sender.lock().await.as_ref() {
                    let err_kind: ChunkErrorKind = packet.payload[0].into();
                    tx.send(Err(err_kind.into())).await.unwrap()
                } else {
                    debug!("Received a LockErr whilst not awaiting a lock");
                }
                *self.lock_sender.lock().await = None;
                return Ok(());
            }
            _ => {}
        }

        // Match user managed
        if self.incoming_handler.lock().await.is_some() {
            let handler = &mut self.incoming_handler.lock().await;
            let handler = handler.as_mut().unwrap();
            let handler_channel = self
                .mdsftp_handler_channel
                .lock()
                .await
                .as_ref()
                .unwrap()
                .clone();
            match packet.packet_type {
                MDSFTPPacketType::FileChunk => {
                    let is_last = packet.payload[0] == 1;
                    let mut payload_bytes = [0; 4];
                    payload_bytes.copy_from_slice(packet.payload[1..5].as_ref());
                    let chunk_id: u32 = u32::from_be_bytes(payload_bytes);
                    handler
                        .handle_file_chunk(handler_channel, &packet.payload[5..], chunk_id, is_last)
                        .await?;
                }
                MDSFTPPacketType::Retrieve => {
                    let chunk_id = Uuid::from_bytes(
                        Bytes::try_from(&packet.payload.as_slice()[0..16])
                            .map_err(MDSFTPError::from)?,
                    );
                    let chunk_buffer =
                        u16::from_be_bytes(packet.payload[16..18].try_into().unwrap());
                    handler
                        .handle_retrieve(handler_channel, chunk_id, chunk_buffer)
                        .await?;
                }
                MDSFTPPacketType::Put => {
                    let mut size_bytes = [0; 8];
                    size_bytes.copy_from_slice(packet.payload[16..24].as_ref());
                    let chunk_id = Uuid::from_bytes(
                        Bytes::try_from(&packet.payload.as_slice()[0..16])
                            .map_err(MDSFTPError::from)?,
                    );
                    let size = u64::from_be_bytes(size_bytes);
                    handler.handle_put(handler_channel, chunk_id, size).await?;
                }
                MDSFTPPacketType::Reserve => {
                    let flags: ReserveFlags = packet.payload[0].into();
                    let size = u64::from_be_bytes(packet.payload[1..9].try_into().unwrap());
                    handler.handle_reserve(handler_channel, size, flags).await?;
                }
                MDSFTPPacketType::LockReq => {
                    let kind = LockKind::from(packet.payload[0]);
                    let chunk_id = Uuid::from_bytes(
                        Bytes::try_from(&packet.payload.as_slice()[1..17])
                            .map_err(MDSFTPError::from)?,
                    );
                    handler
                        .handle_lock_req(handler_channel, chunk_id, kind)
                        .await?;
                }
                MDSFTPPacketType::RecvAck => {
                    let chunk_id = u32::from_be_bytes(packet.payload[0..4].try_into().unwrap());
                    handler
                        .handle_receive_ack(handler_channel, chunk_id)
                        .await?;
                }
                _ => {}
            }
        } else {
            debug!(
                "Received a user managed packet {:?} whilst a handler is not registered",
                packet.packet_type
            );
        }

        Ok(())
    }

    pub(crate) async fn mark_handler_closed(&self, result: MDSFTPResult<()>) {
        let mut sender = self.handler_sender.lock().await;
        if let Some(tx) = sender.as_ref() {
            let _ = tx.send(result).await;
            *sender = None
        }
    }

    pub(crate) async fn interrupt(&self) {
        if let Some(tx) = self.lock_sender.lock().await.as_ref() {
            let _ = tx.send(Err(MDSFTPError::Interrupted)).await;
        }
        if let Some(tx) = self.reserve_sender.lock().await.as_ref() {
            let _ = tx.send(Err(MDSFTPError::Interrupted)).await;
        }
        if let Some(tx) = self.handler_sender.lock().await.as_ref() {
            let _ = tx.send(Err(MDSFTPError::Interrupted)).await;
        }

        if let Some(tx) = self.handler_sender.lock().await.as_ref() {
            let _ = tx.send(Ok(())).await;
            if let Some(handler) = self.incoming_handler.lock().await.as_mut() {
                let _ = handler.handle_interrupt().await;
                let channel = self.mdsftp_handler_channel.lock().await;
                if let Some(channel) = channel.as_ref() {
                    channel.close(Err(MDSFTPError::Interrupted)).await;
                }
            }
        }
    }
}
