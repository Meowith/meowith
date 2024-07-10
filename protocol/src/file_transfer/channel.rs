use std::io::Write;
use std::sync::{Arc, Weak};

use log::debug;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use uuid::{Bytes, Uuid};

use crate::file_transfer::channel_handle::{ChannelAwaitHandle, MDSFTPHandlerChannel};
use crate::file_transfer::data::{ChunkErrorKind, LockKind};
use crate::file_transfer::error::{MDSFTPError, MDSFTPResult};
use crate::file_transfer::handler::ChannelPacketHandler;
use crate::file_transfer::net::packet_reader::PacketReader;
use crate::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::file_transfer::net::packet_writer::PacketWriter;
use crate::file_transfer::net::wire::MDSFTPRawPacket;

pub struct MDSFTPChannel {
    pub(crate) _internal_channel: Arc<InternalMDSFTPChannel>,
}

impl MDSFTPChannel {
    pub async fn request_lock(&self, kind: LockKind, chunk_id: Uuid) -> MDSFTPResult<LockKind> {
        self._internal_channel.request_lock(kind, chunk_id).await
    }

    pub async fn try_reserve(&self, desired: u64) -> MDSFTPResult<Uuid> {
        self._internal_channel.try_reserve(desired).await
    }

    pub async fn send_chunk(&self, is_last: bool, id: u32, content: &[u8]) -> MDSFTPResult<()> {
        self._internal_channel
            .send_chunk(is_last, id, content)
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
    ($buffer:ident $self:ident $passed:ident $name:ident($packet_type:expr, $payload_len:expr, $($param:ident: $ptype:ty),*) -> $ret:ty { $channel_method:block $finish:block }) => {

        #[allow(unused)]
        pub(crate) async fn $name(&self, $($param: $ptype),*) -> $ret {
            let writer = self
                .writer_ref
                .upgrade()
                .expect("Attempted to use a dead connection");
            let mut writer = writer.lock().await;
            let mut $buffer: Vec<u8> = Vec::with_capacity(8);
            let $self = self;

            let mut $passed = $channel_method;

            writer
            .write_raw_packet(MDSFTPRawPacket
            { packet_type: $packet_type, stream_id: $self.id, payload: $buffer, })
            .await.map_err(|_| MDSFTPError::ConnectionError)?;

            return $finish
        }
    };
}

pub(crate) struct InternalMDSFTPChannel {
    pub(crate) id: u32,
    pub(crate) writer_ref: Weak<Mutex<PacketWriter>>,
    pub(crate) reader_ref: Weak<PacketReader>,

    pub(crate) incoming_handler: Mutex<Option<Box<dyn ChannelPacketHandler>>>, // Hate this...
    pub(crate) mdsftp_handler_channel: Mutex<Option<MDSFTPHandlerChannel>>,
    pub(crate) handler_sender: Mutex<Option<Sender<MDSFTPResult<()>>>>,

    lock_sender: Mutex<Option<Sender<MDSFTPResult<LockKind>>>>,
    reserve_sender: Mutex<Option<Sender<MDSFTPResult<Uuid>>>>,
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

    internal_sender_method!(payload_buffer this lock request_lock(MDSFTPPacketType::LockReq, 17, kind: LockKind, chunk_id: Uuid) -> MDSFTPResult<LockKind> {
        {
            payload_buffer.push(kind.into());
            let _ = payload_buffer.write(chunk_id.as_bytes().as_slice());
            let (tx, rx) = mpsc::channel(1);
            *this.lock_sender.lock().await = Some(tx);
            rx
        }
        { lock.recv().await.ok_or(MDSFTPError::Interrupted)? }
    });

    internal_sender_method!(payload_buffer this lock try_reserve(MDSFTPPacketType::Reserve, 8, desired: u64) -> MDSFTPResult<Uuid> {
        {
            let _ = payload_buffer.write(&desired.to_be_bytes());
            let (tx, rx) = mpsc::channel(1);
            *this.reserve_sender.lock().await = Some(tx);
            rx
        }
        { lock.recv().await.ok_or(MDSFTPError::Interrupted)? }
    });

    internal_sender_method!(payload_buffer this none respond_lock_ok(MDSFTPPacketType::LockAcquire, 17, chunk_id: Uuid, kind: LockKind) -> MDSFTPResult<()> {
        {
            let kind: u8 = kind.into();
            payload_buffer.push(kind);
            let _ = payload_buffer.write(chunk_id.as_bytes().as_slice());
        }
        { Ok(()) }
    });

    internal_sender_method!(payload_buffer this none respond_lock_err(MDSFTPPacketType::LockErr, 17, chunk_id: Uuid, kind: LockKind, error_kind: ChunkErrorKind) -> MDSFTPResult<()> {
        {
            let kind: u8 = kind.into();
            let error_kind: u8 = error_kind.into();
            payload_buffer.push(kind | error_kind);
            let _ = payload_buffer.write(chunk_id.as_bytes().as_slice());
        }
        { Ok(()) }
    });

    internal_sender_method!(payload_buffer this none respond_reserve_ok(MDSFTPPacketType::ReserveOk, 16, chunk_id: Uuid) -> MDSFTPResult<()> {
        { let _ = payload_buffer.write(chunk_id.as_bytes().as_slice()); }
        { Ok(()) }
    });

    internal_sender_method!(payload_buffer this none respond_reserve_err(MDSFTPPacketType::ReserveErr, 8, available_space: u64) -> MDSFTPResult<()> {
        { let _ = payload_buffer.write(&available_space.to_be_bytes()); }
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

        // chunk id + content length
        let mut payload_buffer: Vec<u8> = Vec::with_capacity(1 + 4 + content.len());

        payload_buffer.push(if is_last { 0x01 } else { 0x00 });
        for byte in id.to_be_bytes() {
            payload_buffer.push(byte);
        }

        for byte in content {
            payload_buffer.push(*byte);
        }

        writer
            .write_raw_packet(MDSFTPRawPacket {
                packet_type: MDSFTPPacketType::FileChunk,
                stream_id: self.id,
                payload: payload_buffer,
            })
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;

        Ok(())
    }

    pub(crate) async fn handle_packet(&self, packet: MDSFTPRawPacket) {
        // Note: the packet's payload length is pre-validated by the packet reader.
        match packet.packet_type {
            MDSFTPPacketType::ReserveOk => {
                if let Some(tx) = self.reserve_sender.lock().await.as_ref() {
                    let chunk_id = Uuid::from_bytes(
                        Bytes::try_from(packet.payload.as_slice()).expect("NET_ERR"),
                    );
                    tx.send(Ok(chunk_id)).await.unwrap()
                } else {
                    debug!("Received a ReserveOk whilst not awaiting a reservation");
                }
                *self.reserve_sender.lock().await = None;
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
            }
            MDSFTPPacketType::LockAcquire => {
                if let Some(tx) = self.lock_sender.lock().await.as_ref() {
                    tx.send(Ok(packet.payload[0].into())).await.unwrap();
                } else {
                    debug!("Received a LockAcquire whilst not awaiting a lock");
                }
                *self.lock_sender.lock().await = None;
            }
            MDSFTPPacketType::LockErr => {
                if let Some(tx) = self.lock_sender.lock().await.as_ref() {
                    let err_kind: ChunkErrorKind = packet.payload[0].into();
                    tx.send(Err(err_kind.into())).await.unwrap()
                } else {
                    debug!("Received a LockErr whilst not awaiting a lock");
                }
                *self.lock_sender.lock().await = None;
            }
            _ => {}
        }

        // Match user managed TODO: handler err handling
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
                    let _ = handler
                        .handle_file_chunk(handler_channel, &packet.payload[5..], chunk_id, is_last)
                        .await;
                }
                MDSFTPPacketType::Retrieve => {
                    let bytes = Bytes::try_from(&packet.payload.as_slice()[0..16]);
                    if bytes.is_ok() {
                        let chunk_id = Uuid::from_bytes(bytes.unwrap());
                        let _ = handler.handle_retrieve(handler_channel, chunk_id).await;
                    }
                }
                MDSFTPPacketType::Put => {
                    let bytes = Bytes::try_from(&packet.payload.as_slice()[0..16]);
                    if bytes.is_err() {
                        return;
                    }
                    let mut size_bytes = [0; 8];
                    size_bytes.copy_from_slice(packet.payload[16..24].as_ref());
                    let chunk_id = Uuid::from_bytes(bytes.unwrap());
                    let size = u64::from_be_bytes(size_bytes);
                    let _ = handler.handle_put(handler_channel, chunk_id, size).await;
                }
                MDSFTPPacketType::Reserve => {
                    let mut size_bytes = [0; 8];
                    size_bytes.copy_from_slice(packet.payload[0..8].as_ref());
                    let size = u64::from_be_bytes(size_bytes);
                    let _ = handler.handle_reserve(handler_channel, size).await;
                }
                MDSFTPPacketType::LockReq => {
                    let kind = LockKind::from(packet.payload[0]);
                    let bytes = Bytes::try_from(&packet.payload.as_slice()[0..16]);
                    if bytes.is_err() {
                        return;
                    }
                    let chunk_id = Uuid::from_bytes(bytes.unwrap());
                    let _ = handler
                        .handle_lock_req(handler_channel, chunk_id, kind)
                        .await;
                }
                _ => {}
            }
        } else {
            debug!(
                "Received a user managed packet {:?} whilst a handler is not registered",
                packet.packet_type
            );
        }
    }

    pub(crate) async fn mark_handler_closed(&self) {
        if let Some(tx) = self.handler_sender.lock().await.as_ref() {
            let _ = tx.send(Ok(())).await;
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
    }
}
