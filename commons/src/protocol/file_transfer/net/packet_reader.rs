use crate::protocol::file_transfer::channel::{InternalMDSFTPChannel, MDSFTPChannel};
use crate::protocol::file_transfer::connection::ChannelFactory;
use crate::protocol::file_transfer::error::MDSFTPError;
use crate::protocol::file_transfer::handler::PacketHandler;
use crate::protocol::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::protocol::file_transfer::net::packet_writer::PacketWriter;
use crate::protocol::file_transfer::net::wire::{read_header, MDSFTPRawPacket, HEADER_SIZE};
use log::debug;
use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, MutexGuard, RwLock};
use tokio::task::JoinHandle;
use tokio_openssl::SslStream;
use uuid::Uuid;

pub type ConnectionMap = Arc<RwLock<HashMap<u32, Arc<Mutex<InternalMDSFTPChannel>>>>>;
pub type GlobalHandler = Arc<Mutex<Box<dyn PacketHandler>>>;

#[allow(unused)]
pub(crate) struct PacketReader {
    stream: Arc<Mutex<ReadHalf<SslStream<TcpStream>>>>,
    pub(crate) conn_map: ConnectionMap,
    running: Arc<AtomicBool>,
    global_handler: GlobalHandler,
    conn_id: Uuid,
}

impl PacketReader {
    pub fn new(
        stream: Arc<Mutex<ReadHalf<SslStream<TcpStream>>>>,
        global_handler: GlobalHandler,
        conn_id: Uuid,
    ) -> Self {
        PacketReader {
            stream,
            conn_map: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            global_handler,
            conn_id,
        }
    }

    pub async fn close(conn_map: &ConnectionMap) {
        let mut map_mut = conn_map.write().await;
        for internal_channel in map_mut.iter() {
            internal_channel.1.lock().await.interrupt().await;
        }
        map_mut.clear()
    }

    pub fn start(&self, channel_factory: Weak<ChannelFactory>) -> JoinHandle<()> {
        let stream_ref = self.stream.clone();
        let conn_map = self.conn_map.clone();
        let running = self.running.clone();
        let handler = self.global_handler.clone();
        let conn_id = self.conn_id;
        tokio::spawn(async move {
            let mut stream = stream_ref.lock().await;
            let mut header_buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];

            while running.load(Ordering::Relaxed) {
                if stream.read_exact(&mut header_buf).await.is_err() {
                    Self::close(&conn_map).await;
                    break;
                };

                let header = read_header(&header_buf);

                let mut payload: Vec<u8> = Vec::with_capacity(header.payload_size as usize);
                if stream.read_exact(payload.as_mut_slice()).await.is_err() {
                    Self::close(&conn_map).await;
                    break;
                };

                let packet_type = MDSFTPPacketType::try_from(header.packet_id);

                if packet_type.is_err() {
                    Self::close(&conn_map).await;
                    break;
                }
                let packet_type = packet_type.unwrap();
                let raw = MDSFTPRawPacket {
                    packet_type,
                    payload,
                    stream_id: header.stream_id,
                };

                if packet_type.is_system() {
                    Self::handle_global(
                        raw,
                        &conn_map,
                        conn_id,
                        &mut handler.lock().await,
                        &channel_factory,
                    )
                    .await
                } else {
                    let channel: Option<Arc<Mutex<InternalMDSFTPChannel>>> = {
                        let map = conn_map.read().await;
                        map.get(&header.stream_id).cloned()
                    };

                    if channel.is_some() {
                        channel.unwrap().lock().await.handle_packet(raw).await;
                    } else {
                        debug!(
                            "Received a packet for a non-existing channel {}",
                            header.stream_id
                        );
                    }
                }
            }
        })
    }

    async fn handle_global(
        packet: MDSFTPRawPacket,
        conn_map: &ConnectionMap,
        conn_id: Uuid,
        handler: &mut MutexGuard<'_, Box<dyn PacketHandler>>,
        channel_factory: &Weak<ChannelFactory>,
    ) {
        match packet.packet_type {
            MDSFTPPacketType::ChannelOpen => {
                let factory = channel_factory.upgrade();
                if let Some(factory) = factory {
                    let channel = factory.materialize_channel(packet.stream_id, false).await;
                    if let Ok(channel) = channel {
                        handler.channel_open(channel, conn_id);
                    }
                }
            }
            MDSFTPPacketType::ChannelClose => {
                handler.channel_close(packet.stream_id, conn_id);
                let mut map = conn_map.write().await;
                let _ = map.remove(&packet.stream_id);
            }
            MDSFTPPacketType::ChannelErr => handler.channel_err(packet.stream_id, conn_id),
            _ => {}
        }
    }

    pub async fn add_channel(&self, id: u32, channel: Arc<Mutex<InternalMDSFTPChannel>>) {
        let mut map = self.conn_map.write().await;
        map.insert(id, channel);
    }

    pub async fn remove_channel(&self, channel_id: u32) {
        let mut map = self.conn_map.write().await;
        let _ = map.remove(&channel_id);
    }
}
