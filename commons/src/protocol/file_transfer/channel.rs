use std::sync::{Arc, Weak};

use log::debug;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use uuid::{Bytes, Uuid};

use crate::protocol::file_transfer::data::{ChunkErrorKind, LockKind};
use crate::protocol::file_transfer::error::{MDSFTPError, MDSFTPResult};
use crate::protocol::file_transfer::net::packet_reader::PacketReader;
use crate::protocol::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::protocol::file_transfer::net::packet_writer::PacketWriter;
use crate::protocol::file_transfer::net::wire::{MDSFTPRawPacket, HEADER_SIZE};

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
}

impl Drop for MDSFTPChannel {
    fn drop(&mut self) {
        let internal_ref = self._internal_channel.clone();
        tokio::spawn(async move {
            internal_ref.lock().await.cleanup().await;
        });
    }
}

#[allow(unused)]
pub(crate) struct InternalMDSFTPChannel {
    pub(crate) id: u32,
    pub(crate) writer_ref: Weak<Mutex<PacketWriter>>,
    pub(crate) reader_ref: Weak<PacketReader>,

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
        match packet.packet_type {
            MDSFTPPacketType::FileChunk => {}
            MDSFTPPacketType::Retrieve => {}
            MDSFTPPacketType::Put => {}
            MDSFTPPacketType::Reserve => {}
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
            MDSFTPPacketType::LockReq => {}
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
