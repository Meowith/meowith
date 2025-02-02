use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

use log::{error, trace, warn};
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, MutexGuard, RwLock};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_rustls::TlsStream;
use uuid::Uuid;

use crate::mdsftp::channel::InternalMDSFTPChannel;
use crate::mdsftp::connection::ChannelFactory;
use crate::mdsftp::handler::PacketHandler;
use crate::mdsftp::net::packet_type::MDSFTPPacketType;
use crate::mdsftp::net::validate::PreValidate;
use crate::mdsftp::net::wire::{read_header, MDSFTPRawPacket, HEADER_SIZE};
use commons::error::mdsftp_error::MDSFTPError;

pub type ConnectionMap = Arc<RwLock<HashMap<u32, Arc<InternalMDSFTPChannel>>>>;
pub type GlobalHandler = Arc<Mutex<Box<dyn PacketHandler>>>;

pub(crate) struct PacketReader {
    pub(crate) stream: Arc<Mutex<ReadHalf<TlsStream<TcpStream>>>>,
    pub(crate) conn_map: ConnectionMap,
    pub(crate) running: Arc<AtomicBool>,
    global_handler: GlobalHandler,
    conn_id: Uuid,
    channel_count: Arc<AtomicUsize>,
    last_read: Arc<Mutex<Instant>>,
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
            last_read: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub(crate) async fn close_map(
        conn_map: &ConnectionMap,
        channels: &Arc<AtomicUsize>,
        running: &Arc<AtomicBool>,
    ) {
        let mut map_mut = conn_map.write().await;
        for internal_channel in map_mut.iter() {
            internal_channel.1.interrupt().await;
        }
        map_mut.clear();
        trace!("Close map call");
        channels.store(0, Ordering::SeqCst);
        running.store(false, Ordering::SeqCst);
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

            trace!("Packet reader {conn_id} start");

            while running.load(Ordering::Relaxed) {
                trace!("Packet reader {conn_id} <- awaiting head");
                if let Err(e) = stream.read_exact(&mut header_buf).await {
                    error!("Header read failed {e} {conn_id}");
                    Self::close_map(&conn_map, &channels, &running).await;
                    break;
                };

                let header = read_header(&header_buf);
                let packet_type = MDSFTPPacketType::try_from(header.packet_id);
                if packet_type.is_err() {
                    error!("Packet type read failed");
                    Self::close_map(&conn_map, &channels, &running).await;
                    break;
                }
                let packet_type = packet_type.unwrap();
                if !packet_type.pre_validate(&header) {
                    error!("Packet pre-validate failed");
                    Self::close_map(&conn_map, &channels, &running).await;
                    break;
                }

                trace!(
                    "Packet reader {conn_id} <- awaiting payload type={packet_type:?} len={}",
                    header.payload_size
                );
                let mut payload = vec![0u8; header.payload_size as usize];
                if let Err(e) = stream.read_exact(&mut payload).await {
                    error!("Packet payload read failed {e}");
                    Self::close_map(&conn_map, &channels, &running).await;
                    break;
                };

                if payload.len() != header.payload_size as usize {
                    error!("Packet payload read failed");
                    Self::close_map(&conn_map, &channels, &running).await;
                    break;
                }

                let raw = MDSFTPRawPacket {
                    packet_type,
                    payload,
                    stream_id: header.stream_id,
                };

                {
                    let mut last_read = last_read.lock().await;
                    *last_read = Instant::now();
                }

                if packet_type.is_system() {
                    trace!("Packet reader {conn_id} awaiting handle global");
                    Self::handle_global(
                        raw,
                        &conn_map,
                        conn_id,
                        &mut handler.lock().await,
                        &channel_factory,
                    )
                    .await
                } else {
                    trace!("Packet reader {conn_id} awaiting internal ref get");
                    let channel: Option<Arc<InternalMDSFTPChannel>> = {
                        let map = conn_map.read().await;
                        map.get(&header.stream_id).cloned()
                    };

                    if channel.is_some() {
                        trace!("Packet reader {conn_id} awaiting channel handle packet");
                        let channel = channel.unwrap();
                        match channel.handle_packet(raw).await {
                            Err(MDSFTPError::ConnectionError) => {
                                error!("Connection poisoned. Closing.");
                                Self::close_map(&conn_map, &channels, &running).await;
                                break;
                            }
                            Err(e) => {
                                warn!("Channel handler error {e} {conn_id}");
                            }
                            _ => {}
                        };
                    } else {
                        trace!(
                            "Received a packet for a non-existing channel {}",
                            header.stream_id
                        );
                    }
                }
            }

            trace!("Reader loop close");
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
                let mut map = conn_map.write().await;
                let removed = map.remove(&packet.stream_id);
                if let Some(removed) = removed {
                    trace!("removed connection due to remote {}", &packet.stream_id);
                    removed.interrupt().await;
                    handler.channel_close(packet.stream_id, conn_id).await;
                } else {
                    trace!(
                        "received a close packet for a non existing channel {}, {:?}",
                        &packet.stream_id,
                        map.keys()
                    );
                }
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
        if let Some(removed) = removed {
            trace!("removed connection due to drop {}", channel_id);
            removed.interrupt().await;
            self.channel_count.fetch_sub(1, Ordering::Relaxed);
        } else {
            trace!(
                "non existing channel tried to close itself {}, {:?}",
                channel_id,
                map.keys()
            );
        }
    }

    pub(crate) fn channel_count(&self) -> usize {
        self.channel_count.load(Ordering::Relaxed)
    }

    pub(crate) async fn close(&self) {
        trace!("Closing the packet reader.");
        Self::close_map(&self.conn_map, &self.channel_count, &self.running).await;
    }

    pub(crate) async fn last_read(&self) -> Instant {
        *self.last_read.lock().await
    }
}
