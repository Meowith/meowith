use std::sync::{Arc, Weak};

use log::debug;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use uuid::{Bytes, Uuid};

use crate::file_transfer::data::{ChunkErrorKind, LockKind};
use crate::file_transfer::error::{MDSFTPError, MDSFTPResult};
use crate::file_transfer::handler::ChannelPacketHandler;
use crate::file_transfer::net::packet_reader::PacketReader;
use crate::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::file_transfer::net::packet_writer::PacketWriter;
use crate::file_transfer::net::wire::{MDSFTPRawPacket, HEADER_SIZE};

pub struct MDSFTPChannel {
    pub(crate) _internal_channel: Arc<Mutex<InternalMDSFTPChannel>>,
}

impl MDSFTPChannel {
    pub async fn request_lock(&self, kind: LockKind, chunk_id: Uuid) -> MDSFTPResult<LockKind> {
        self._internal_channel
            .lock()
            .await
            .request_lock(kind, chunk_id)
            .await
    }

    pub async fn try_reserve(&self, desired: u64) -> MDSFTPResult<Uuid> {
        self._internal_channel
            .lock()
            .await
            .try_reserve(desired)
            .await
    }

    pub async fn send_chunk(&self, is_last: bool, id: u32, content: &[u8]) -> MDSFTPResult<()> {
        self._internal_channel
            .lock()
            .await
            .send_chunk(is_last, id, content)
            .await
    }

    pub async fn set_incoming_handler(&self, handler: Box<dyn ChannelPacketHandler>) {
        self._internal_channel.lock().await.incoming_handler = Some(Arc::new(Mutex::new(handler)));
    }
}

impl Drop for MDSFTPChannel {
    fn drop(&mut self) {
        let internal_ref = self._internal_channel.clone();
        tokio::spawn(async move {
            internal_ref.lock().await.cleanup().await;
        });
    }
}

pub(crate) struct InternalMDSFTPChannel {
    pub(crate) id: u32,
    pub(crate) writer_ref: Weak<Mutex<PacketWriter>>,
    pub(crate) reader_ref: Weak<PacketReader>,

    pub(crate) incoming_handler: Option<Arc<Mutex<Box<dyn ChannelPacketHandler>>>>,

    lock_sender: Option<Sender<MDSFTPResult<LockKind>>>,
    reserve_sender: Option<Sender<MDSFTPResult<Uuid>>>,
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
            lock_sender: None,
            reserve_sender: None,
            incoming_handler: None,
        }
    }

    /// Unregister the listener
    pub(super) async fn cleanup(&self) {
        let reader = self.reader_ref.upgrade();
        if reader.is_some() {
            reader.unwrap().remove_channel(self.id).await;
        }
    }

    pub(super) async fn request_lock(
        &mut self,
        kind: LockKind,
        chunk_id: Uuid,
    ) -> MDSFTPResult<LockKind> {
        let writer = self
            .writer_ref
            .upgrade()
            .expect("Attempted to use a dead connection");
        let mut writer = writer.lock().await;
        //header size + flags size + chunk id size
        let mut payload_buffer: Vec<u8> = Vec::with_capacity(HEADER_SIZE + 1 + 16);

        payload_buffer.push(kind.into());
        for byte in chunk_id.as_bytes() {
            payload_buffer.push(byte.to_owned());
        }

        writer
            .write_raw_packet(MDSFTPRawPacket {
                packet_type: MDSFTPPacketType::LockReq,
                stream_id: self.id,
                payload: payload_buffer,
            })
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;

        let (tx, mut rx) = mpsc::channel(1);
        self.lock_sender = Some(tx);
        rx.recv().await.ok_or(MDSFTPError::Interrupted)?
    }

    pub(super) async fn try_reserve(&mut self, desired: u64) -> MDSFTPResult<Uuid> {
        let writer = self
            .writer_ref
            .upgrade()
            .expect("Attempted to use a dead connection");
        let mut writer = writer.lock().await;

        //header size + desired
        let mut payload_buffer: Vec<u8> = Vec::with_capacity(HEADER_SIZE + 8);

        for byte in desired.to_be_bytes() {
            payload_buffer.push(byte);
        }

        writer
            .write_raw_packet(MDSFTPRawPacket {
                packet_type: MDSFTPPacketType::Reserve,
                stream_id: self.id,
                payload: payload_buffer,
            })
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;

        let (tx, mut rx) = mpsc::channel(1);
        self.reserve_sender = Some(tx);
        rx.recv().await.ok_or(MDSFTPError::Interrupted)?
    }

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

        //header size + flags + chunk id + content length
        let mut payload_buffer: Vec<u8> = Vec::with_capacity(HEADER_SIZE + 1 + 4 + content.len());

        payload_buffer.push(if is_last { 0x01 } else { 0x00 });
        for byte in id.to_be_bytes() {
            payload_buffer.push(byte);
        }

        for byte in content {
            payload_buffer.push(*byte);
        }

        writer
            .write_raw_packet(MDSFTPRawPacket {
                packet_type: MDSFTPPacketType::Reserve,
                stream_id: self.id,
                payload: payload_buffer,
            })
            .await
            .map_err(|_| MDSFTPError::ConnectionError)?;

        Ok(())
    }

    pub(crate) async fn handle_packet(&mut self, packet: MDSFTPRawPacket) {
        // Note: the packet's payload length is pre-validated by the packet reader.
        match packet.packet_type {
            MDSFTPPacketType::ReserveOk => {
                if let Some(tx) = &self.reserve_sender {
                    let chunk_id = Uuid::from_bytes(
                        Bytes::try_from(packet.payload.as_slice()).expect("NET_ERR"),
                    );
                    tx.send(Ok(chunk_id)).await.unwrap()
                } else {
                    debug!("Received a ReserveOk whilst not awaiting a reservation");
                }
                self.reserve_sender = None
            }
            MDSFTPPacketType::ReserveErr => {
                if let Some(tx) = &self.reserve_sender {
                    let mut payload_bytes = [0; 8];
                    payload_bytes.copy_from_slice(packet.payload.as_slice());

                    let max_space: u64 = u64::from_be_bytes(payload_bytes);
                    tx.send(Err(MDSFTPError::ReserveError(max_space)))
                        .await
                        .unwrap()
                } else {
                    debug!("Received a ReserveOk whilst not awaiting a reservation");
                }
                self.reserve_sender = None
            }
            MDSFTPPacketType::LockAcquire => {
                if let Some(tx) = &self.lock_sender {
                    tx.send(Ok(packet.payload[0].into())).await.unwrap()
                } else {
                    debug!("Received a LockAcquire whilst not awaiting a lock");
                }
                self.lock_sender = None
            }
            MDSFTPPacketType::LockErr => {
                if let Some(tx) = &self.lock_sender {
                    let err_kind: ChunkErrorKind = packet.payload[0].into();
                    tx.send(Err(err_kind.into())).await.unwrap()
                } else {
                    debug!("Received a LockErr whilst not awaiting a lock");
                }
                self.lock_sender = None
            }
            _ => {}
        }

        // Match user managed
        if self.incoming_handler.is_some() {
            let handler = &mut self.incoming_handler.as_ref().unwrap().lock().await;
            match packet.packet_type {
                MDSFTPPacketType::FileChunk => {
                    let is_last = packet.payload[0] == 1;
                    let mut payload_bytes = [0; 4];
                    payload_bytes.copy_from_slice(packet.payload[1..5].as_ref());
                    let chunk_id: u32 = u32::from_be_bytes(payload_bytes);
                    handler.handle_file_chunk(&packet.payload[5..], chunk_id, is_last);
                }
                MDSFTPPacketType::Retrieve => {
                    let bytes = Bytes::try_from(&packet.payload.as_slice()[0..16]);
                    if bytes.is_ok() {
                        let chunk_id = Uuid::from_bytes(bytes.unwrap());
                        handler.handle_retrieve(chunk_id);
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
                    handler.handle_put(chunk_id, size);
                }
                MDSFTPPacketType::Reserve => {
                    let mut size_bytes = [0; 8];
                    size_bytes.copy_from_slice(packet.payload[0..8].as_ref());
                    let size = u64::from_be_bytes(size_bytes);
                    handler.handle_reserve(size);
                }
                MDSFTPPacketType::LockReq => {
                    let kind = LockKind::from(packet.payload[0]);
                    let bytes = Bytes::try_from(&packet.payload.as_slice()[0..16]);
                    if bytes.is_err() {
                        return;
                    }
                    let chunk_id = Uuid::from_bytes(bytes.unwrap());
                    handler.handle_lock_req(chunk_id, kind);
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

    pub(crate) async fn interrupt(&self) {
        if let Some(tx) = &self.lock_sender {
            tx.send(Err(MDSFTPError::Interrupted)).await.unwrap()
        }
        if let Some(tx) = &self.reserve_sender {
            tx.send(Err(MDSFTPError::Interrupted)).await.unwrap()
        }
    }
}
