use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use crate::file_transfer::channel::InternalMDSFTPChannel;
use crate::file_transfer::connection::ChannelFactory;
use crate::file_transfer::handler::PacketHandler;
use crate::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::file_transfer::net::validate::PreValidate;
use crate::file_transfer::net::wire::{read_header, MDSFTPRawPacket, HEADER_SIZE};
use chrono::{DateTime, Utc};
use log::{debug, warn};
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, MutexGuard, RwLock};
use tokio::task::JoinHandle;
use tokio_rustls::TlsStream;
use uuid::Uuid;

pub type ConnectionMap = Arc<RwLock<HashMap<u32, Arc<InternalMDSFTPChannel>>>>;
pub type GlobalHandler = Arc<Mutex<Box<dyn PacketHandler>>>;

pub(crate) struct PacketReader {
    pub(crate) stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
    pub(crate) conn_map: ConnectionMap,
    running: Arc<AtomicBool>,
    global_handler: GlobalHandler,
    conn_id: Uuid,
    channel_count: Arc<AtomicUsize>,
    last_read: Arc<Mutex<DateTime<Utc>>>,
}

impl PacketReader {
    pub(crate) fn new(
        stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
        global_handler: GlobalHandler,
        conn_id: Uuid,
    ) -> Self {
        PacketReader {
            stream,
            conn_map: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            global_handler,
            conn_id,
            channel_count: Arc::new(AtomicUsize::new(0)),
            last_read: Arc::new(Mutex::new(DateTime::<Utc>::MIN_UTC)),
        }
    }

    pub(crate) async fn close_map(conn_map: &ConnectionMap, channels: &Arc<AtomicUsize>) {
        let mut map_mut = conn_map.write().await;
        for internal_channel in map_mut.iter() {
            internal_channel.1.interrupt().await;
        }
        map_mut.clear();
        channels.store(0, Ordering::SeqCst);
    }

    pub(crate) fn start(&self, channel_factory: Weak<ChannelFactory>) -> JoinHandle<()> {
        let stream_ref = self.stream.clone();
        let conn_map = self.conn_map.clone();
        let running = self.running.clone();
        let handler = self.global_handler.clone();
        let conn_id = self.conn_id;
        let channels = self.channel_count.clone();
        let last_read = self.last_read.clone();
        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let mut stream = stream_ref.lock().await;
            let mut header_buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];

            while running.load(Ordering::Relaxed) {
                if stream.read_exact(&mut header_buf).await.is_err() {
                    Self::close_map(&conn_map, &channels).await;
                    break;
                };

                let header = read_header(&header_buf);
                let packet_type = MDSFTPPacketType::try_from(header.packet_id);
                if packet_type.is_err() {
                    Self::close_map(&conn_map, &channels).await;
                    break;
                }
                let packet_type = packet_type.unwrap();
                if !packet_type.pre_validate(&header) {
                    Self::close_map(&conn_map, &channels).await;
                    break;
                }

                let mut payload = vec![0u8; header.payload_size as usize];
                if stream.read_exact(&mut payload).await.is_err() {
                    Self::close_map(&conn_map, &channels).await;
                    break;
                };

                if payload.len() != header.payload_size as usize {
                    Self::close_map(&conn_map, &channels).await;
                    break;
                }

                let raw = MDSFTPRawPacket {
                    packet_type,
                    payload,
                    stream_id: header.stream_id,
                };

                {
                    let mut last_read = last_read.lock().await;
                    *last_read = Utc::now();
                }

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
                    let channel: Option<Arc<InternalMDSFTPChannel>> = {
                        let map = conn_map.read().await;
                        map.get(&header.stream_id).cloned()
                    };

                    if channel.is_some() {
                        let channel = channel.unwrap();
                        channel.handle_packet(raw).await;
                    } else {
                        debug!(
                            "Received a packet for a non-existing channel {}",
                            header.stream_id
                        );
                    }
                }
            }

            debug!("Reader loop close");
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
                        handler.channel_incoming(channel, conn_id).await;
                    }
                }
            }
            MDSFTPPacketType::ChannelClose => {
                handler.channel_close(packet.stream_id, conn_id).await;
                let mut map = conn_map.write().await;
                let _ = map.remove(&packet.stream_id);
            }
            MDSFTPPacketType::ChannelErr => handler.channel_err(packet.stream_id, conn_id).await,
            _ => {}
        }
    }

    pub(crate) async fn add_channel(&self, id: u32, channel: Arc<InternalMDSFTPChannel>) {
        let mut map = self.conn_map.write().await;
        let entry = map.entry(id);
        match entry {
            Entry::Occupied(_) => {
                warn!("Duplicate channel ID {id}")
            }
            Entry::Vacant(entry) => {
                entry.insert(channel);
                self.channel_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub(crate) async fn remove_channel(&self, channel_id: u32) {
        let mut map = self.conn_map.write().await;
        let removed = map.remove(&channel_id);
        if removed.is_some() {
            self.channel_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn channel_count(&self) -> usize {
        self.channel_count.load(Ordering::Relaxed)
    }

    pub(crate) async fn close(&self) {
        debug!("Closing the packet reader.");
        self.running.store(false, Ordering::SeqCst);
        Self::close_map(&self.conn_map, &self.channel_count).await;
    }

    pub(crate) async fn last_read(&self) -> DateTime<Utc> {
        *self.last_read.lock().await
    }
}
