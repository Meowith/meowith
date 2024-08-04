use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use multimap::MultiMap;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time;
use tokio_rustls::TlsStream;
use uuid::Uuid;

use commons::context::microservice_request_context::NodeAddrMap;

use crate::mdsftp::authenticator::ConnectionAuthContext;
use crate::mdsftp::channel::MDSFTPChannel;
use crate::mdsftp::connection::MDSFTPConnection;
use crate::mdsftp::error::{MDSFTPError, MDSFTPResult};
use crate::mdsftp::handler::PacketHandler;
use crate::mdsftp::net::packet_reader::GlobalHandler;

pub type PacketHandlerRef = Arc<Mutex<Box<dyn PacketHandler>>>;
static STALE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct MDSFTPPoolConfigHolder {
    pub fragment_size: u32,
    pub buffer_size: u16,
}

impl Default for MDSFTPPoolConfigHolder {
    fn default() -> Self {
        MDSFTPPoolConfigHolder {
            fragment_size: u16::MAX as u32,
            buffer_size: 16,
        }
    }
}

#[derive(Clone)]
pub struct MDSFTPPool {
    pub(crate) _internal_pool: Arc<Mutex<InternalMDSFTPPool>>,
    pub cfg: MDSFTPPoolConfigHolder,
}

impl MDSFTPPool {
    pub fn new(
        connection_auth_context: Arc<ConnectionAuthContext>,
        node_addr_map: NodeAddrMap,
        cfg: MDSFTPPoolConfigHolder,
    ) -> Self {
        MDSFTPPool {
            _internal_pool: Arc::new(Mutex::new(InternalMDSFTPPool::new(
                connection_auth_context,
                node_addr_map,
            ))),
            cfg,
        }
    }

    pub async fn set_packet_handler(&mut self, handler: PacketHandlerRef) {
        self._internal_pool.lock().await.set_packet_handler(handler);
    }

    pub async fn channel(&self, node_id: &Uuid) -> MDSFTPResult<MDSFTPChannel> {
        self._internal_pool.lock().await.channel(node_id).await
    }

    pub async fn shutdown(&self) {
        self._internal_pool.lock().await.close().await
    }

    pub async fn watch_stale(&self) {
        self._internal_pool.lock().await.watch_stale().await
    }
}

pub(crate) struct InternalMDSFTPPool {
    connection_map: Arc<RwLock<MultiMap<Uuid, MDSFTPConnection>>>,
    packet_handler: Option<GlobalHandler>,
    shutting_down: AtomicBool,
    connection_auth_context: Arc<ConnectionAuthContext>,
    node_addr_map: NodeAddrMap,
    stale_conn_watcher: Option<JoinHandle<()>>,
}

#[allow(unused)]
impl InternalMDSFTPPool {
    fn new(
        connection_auth_context: Arc<ConnectionAuthContext>,
        node_addr_map: NodeAddrMap,
    ) -> Self {
        InternalMDSFTPPool {
            connection_auth_context,
            connection_map: Arc::new(RwLock::new(MultiMap::new())),
            packet_handler: None,
            shutting_down: AtomicBool::new(false),
            stale_conn_watcher: None,
            node_addr_map,
        }
    }

    pub(crate) async fn watch_stale(&mut self) {
        let conn_map = self.connection_map.clone();

        if self.stale_conn_watcher.is_none() {
            self.stale_conn_watcher = Some(tokio::spawn(async move {
                let mut interval = time::interval(STALE_TIMEOUT);

                loop {
                    interval.tick().await;
                    let mut map = conn_map.write().await;
                    let mut mark = MultiMap::new();
                    for (id, conn) in map.iter() {
                        if conn.last_access().await.elapsed() > STALE_TIMEOUT
                            && conn.safe_to_close()
                        {
                            conn.close();
                            mark.insert(*id, conn.local_id())
                        }
                    }

                    for (id, sweep) in mark {
                        let mut vec = map.remove(&id);
                        if let Some(mut vec) = vec {
                            vec.retain(|c| !sweep.contains(&c.local_id()));
                            if !vec.is_empty() {
                                map.insert_many(id, vec);
                            }
                        }
                    }
                }
            }))
        }
    }

    async fn get_connection(&self, node_id: &Uuid) -> MDSFTPResult<MDSFTPConnection> {
        let mut map_mut = self.connection_map.write().await;
        let cached = map_mut.get_vec(node_id).cloned();

        if let Some(connections) = cached {
            if !connections.is_empty() {
                return Ok(connections
                    .iter()
                    .min_by_key(|c| c.channel_count())
                    .unwrap()
                    .clone());
            }
        }

        let packet_handler = self
            .packet_handler
            .as_ref()
            .ok_or(MDSFTPError::NoPacketHandler)?;

        let new_connection = self
            .create_connection(node_id, packet_handler.clone())
            .await?;
        map_mut.insert(*node_id, new_connection.clone());

        Ok(new_connection)
    }

    pub(crate) async fn channel(&self, node_id: &Uuid) -> MDSFTPResult<MDSFTPChannel> {
        let conn = self.get_connection(node_id).await?;
        conn.create_channel().await
    }

    pub(crate) fn set_packet_handler(&mut self, handler: PacketHandlerRef) {
        self.packet_handler = Some(handler);
    }

    async fn create_connection(
        &self,
        target: &Uuid,
        handler: GlobalHandler,
    ) -> MDSFTPResult<MDSFTPConnection> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(MDSFTPError::ShuttingDown);
        }

        let port = &self.connection_auth_context.port;

        let map = self.node_addr_map.read().await;
        let node = map.get(target).cloned().ok_or(MDSFTPError::NoSuchNode)?;

        MDSFTPConnection::new(
            SocketAddr::new(
                IpAddr::from_str(node.as_str()).map_err(|_| MDSFTPError::AddressResolutionError)?,
                *port,
            ),
            &self.connection_auth_context,
            *target,
            handler,
        )
        .await
    }

    pub(crate) async fn add_connection(
        &self,
        id: Uuid,
        conn: TlsStream<TcpStream>,
    ) -> MDSFTPResult<()> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(MDSFTPError::ShuttingDown);
        }
        let mut map = self.connection_map.write().await;

        let packet_handler = self
            .packet_handler
            .as_ref()
            .ok_or(MDSFTPError::NoPacketHandler)?;
        let conn_handle = MDSFTPConnection::from_conn(id, packet_handler.clone(), conn).await?;
        map.insert(id, conn_handle);

        Ok(())
    }

    pub(crate) async fn close(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        if let Some(handle) = &self.stale_conn_watcher {
            handle.abort();
        }
        let mut con_map = self.connection_map.write().await;

        for entry in con_map.iter_mut() {
            entry.1.close().await
        }

        con_map.clear()
    }
}

impl Drop for MDSFTPPool {
    fn drop(&mut self) {
        let internal = self._internal_pool.clone();
        tokio::spawn(async move {
            internal.lock().await.close().await;
        });
    }
}
