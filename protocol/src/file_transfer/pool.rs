use crate::file_transfer::channel::MDSFTPChannel;
use crate::file_transfer::connection::MDSFTPConnection;
use crate::file_transfer::error::{MDSFTPError, MDSFTPResult};
use crate::file_transfer::handler::PacketHandler;
use crate::file_transfer::net::packet_reader::GlobalHandler;
use commons::context::microservice_request_context::MicroserviceRequestContext;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio_rustls::TlsStream;
use uuid::Uuid;

#[derive(Clone)]
pub struct MDSFTPPool {
    _internal_pool: Arc<InternalMDSFTPPool>,
}

impl MDSFTPPool {
    pub fn new(req_ctx: Arc<MicroserviceRequestContext>) -> Self {
        MDSFTPPool {
            _internal_pool: Arc::new(InternalMDSFTPPool::new(req_ctx)),
        }
    }

    pub async fn channel(&self, node_id: &Uuid) -> MDSFTPResult<MDSFTPChannel> {
        self._internal_pool.channel(node_id).await
    }

    pub async fn shutdown(&self) {
        todo!()
    }
}

struct InternalMDSFTPPool {
    req_ctx: Arc<MicroserviceRequestContext>,
    connection_map: RwLock<HashMap<Uuid, MDSFTPConnection>>,
    packet_handler: Option<GlobalHandler>,
}

impl InternalMDSFTPPool {
    fn new(req_ctx: Arc<MicroserviceRequestContext>) -> Self {
        InternalMDSFTPPool {
            req_ctx,
            connection_map: RwLock::new(HashMap::new()),
            packet_handler: None,
        }
    }

    async fn get_connection(&self, node_id: &Uuid) -> MDSFTPResult<MDSFTPConnection> {
        let mut map_mut = self.connection_map.write().await;
        let cached = map_mut.get(node_id).cloned();

        if let Some(connection) = cached {
            return Ok(connection);
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

    pub(crate) fn set_packet_handler(&mut self, handler: Box<dyn PacketHandler>) {
        self.packet_handler = Some(Arc::new(Mutex::new(handler)));
    }

    async fn create_connection(
        &self,
        target: &Uuid,
        handler: GlobalHandler,
    ) -> MDSFTPResult<MDSFTPConnection> {
        let port = 6969; // TODO

        let map = self.req_ctx.node_addr.read().await;
        let node = map.get(target).cloned().ok_or(MDSFTPError::NoSuchNode)?;

        MDSFTPConnection::new(
            SocketAddr::new(
                IpAddr::from_str(node.as_str()).map_err(|_| MDSFTPError::AddressResolutionError)?,
                port,
            ),
            &self.req_ctx.root_x509,
            *target,
            &self.req_ctx.access_token,
            handler,
        )
        .await
    }

    async fn add_connection(conn: TlsStream<TcpStream>) {
        todo!()
    }
}
