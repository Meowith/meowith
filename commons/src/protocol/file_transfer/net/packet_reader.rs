use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use log::debug;
use tokio::io::{AsyncReadExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio_openssl::SslStream;
use crate::protocol::file_transfer::channel::InternalMDSFTPChannel;
use crate::protocol::file_transfer::net::packet_type::MDSFTPPacketType;
use crate::protocol::file_transfer::net::wire::{HEADER_SIZE, MDSFTPRawPacket, read_header};

pub type ConnectionMap = Arc<RwLock<HashMap<u32, Arc<InternalMDSFTPChannel>>>>;

#[allow(unused)]
pub(crate) struct PacketReader
{
    stream: Arc<Mutex<ReadHalf<SslStream<TcpStream>>>>,
    join_handle: Option<JoinHandle<()>>,
    pub(crate) conn_map: ConnectionMap,
    running: Arc<AtomicBool>,
}

impl PacketReader {
    pub fn new(stream: Arc<Mutex<ReadHalf<SslStream<TcpStream>>>>) -> Self {
        let mut reader = PacketReader {
            stream,
            join_handle: None,
            conn_map: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
        };
        reader.start();
        reader
    }

    pub async fn close(conn_map: &ConnectionMap) {
        let mut map_mut = conn_map.write().await;
        for internal_channel in map_mut.iter() {
            internal_channel.1.interrupt().await;
        }
        map_mut.clear()
    }

    pub fn start(&mut self) {
        let stream_ref = self.stream.clone();
        let conn_map = self.conn_map.clone();
        let running = self.running.clone();
        self.join_handle = Some(tokio::spawn(async move {
            let mut stream = stream_ref.lock().await;
            let mut header_buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];

            while running.load(Ordering::Relaxed) {
                if stream.read_exact(&mut header_buf).await.is_err() {
                    Self::close(&conn_map).await;
                    break;
                };

                let header = read_header(&header_buf);

                let mut payload: Vec<u8> = Vec::with_capacity(header.payload_size as usize);
                if stream.read_exact(payload.as_mut_slice()).await.is_err()
                {
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
                };

                if packet_type.is_system() {

                } else {
                    let channel: Option<Arc<InternalMDSFTPChannel>> = {
                        let map = conn_map.read().await;
                        map.get(&header.stream_id).cloned()
                    };

                    if channel.is_some() {
                        channel.unwrap().handle_packet(raw).await;
                    } else {
                        debug!("Received a packet for a non-existing channel {}", header.stream_id);
                    }
                }
            }
        }));
    }

    pub async fn add_channel(&self, channel: Arc<InternalMDSFTPChannel>) {
        let mut map = self.conn_map.write().await;
        map.insert(channel.id, channel);
    }

    pub async fn remove_channel(&self, channel_id: u32) {
        let mut map = self.conn_map.write().await;
        let _ = map.remove(&channel_id);
    }
}
