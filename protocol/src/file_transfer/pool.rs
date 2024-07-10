use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use multimap::MultiMap;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio_rustls::TlsStream;
use uuid::Uuid;

use crate::file_transfer::authenticator::ConnectionAuthContext;
use crate::file_transfer::channel::MDSFTPChannel;
use crate::file_transfer::connection::MDSFTPConnection;
use crate::file_transfer::error::{MDSFTPError, MDSFTPResult};
use crate::file_transfer::handler::PacketHandler;
use crate::file_transfer::net::packet_reader::GlobalHandler;
use commons::context::microservice_request_context::NodeAddrMap;

pub type PacketHandlerRef = Arc<Mutex<Box<dyn PacketHandler>>>;

#[derive(Clone)]
pub struct MDSFTPPool {
    pub(crate) _internal_pool: Arc<Mutex<InternalMDSFTPPool>>,
}

// TODO: auto close stale conn's

impl MDSFTPPool {
    pub fn new(
        connection_auth_context: Arc<ConnectionAuthContext>,
        node_addr_map: NodeAddrMap,
    ) -> Self {
        MDSFTPPool {
            _internal_pool: Arc::new(Mutex::new(InternalMDSFTPPool::new(
                connection_auth_context,
                node_addr_map,
            ))),
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
}

pub(crate) struct InternalMDSFTPPool {
    connection_map: RwLock<MultiMap<Uuid, MDSFTPConnection>>,
    packet_handler: Option<GlobalHandler>,
    shutting_down: AtomicBool,
    connection_auth_context: Arc<ConnectionAuthContext>,
    node_addr_map: NodeAddrMap,
}

#[allow(unused)]
impl InternalMDSFTPPool {
    fn new(
        connection_auth_context: Arc<ConnectionAuthContext>,
        node_addr_map: NodeAddrMap,
    ) -> Self {
        InternalMDSFTPPool {
            connection_auth_context,
            connection_map: RwLock::new(MultiMap::new()),
            packet_handler: None,
            shutting_down: AtomicBool::new(false),
            node_addr_map,
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
            return Err(MDSFTPError::Interrupted);
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
            return Err(MDSFTPError::Interrupted);
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
        let mut con_map = self.connection_map.write().await;

        for entry in con_map.iter_mut() {
            entry.1.close().await
        }

        con_map.clear()
    }
}
