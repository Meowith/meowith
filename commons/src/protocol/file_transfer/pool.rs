use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::context::microservice_request_context::MicroserviceRequestContext;
use crate::protocol::file_transfer::channel::MDSFTPChannel;
use crate::protocol::file_transfer::connection::MDSFTPConnection;
use crate::protocol::file_transfer::error::{MDSFTPError, MDSFTPResult};

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
}

impl InternalMDSFTPPool {
    fn new(req_ctx: Arc<MicroserviceRequestContext>) -> Self {
        InternalMDSFTPPool {
            req_ctx,
            connection_map: RwLock::new(HashMap::new()),
        }
    }

    async fn get_connection(&self, node_id: &Uuid) -> MDSFTPResult<MDSFTPConnection> {
        let mut map_mut = self.connection_map.write().await;
        let cached = map_mut.get(node_id).cloned();

        if let Some(connection) = cached {
            return Ok(connection);
        }

        let new_connection = self.create_connection(node_id).await?;
        map_mut.insert(*node_id, new_connection.clone());

        Ok(new_connection)
    }

    pub(crate) async fn channel(&self, node_id: &Uuid) -> MDSFTPResult<MDSFTPChannel> {
        let conn = self.get_connection(node_id).await?;
        conn.create_channel().await
    }

    async fn create_connection(&self, target: &Uuid) -> MDSFTPResult<MDSFTPConnection> {
        let port = 6969; // TODO

        let map = self.req_ctx.node_addr.read().await;
        let node = map.get(target).cloned().ok_or(MDSFTPError::NoSuchNode)?;

        MDSFTPConnection::new(
            SocketAddr::new(
                IpAddr::from_str(node.as_str()).map_err(|_| MDSFTPError::AddressResolutionError)?,
                port,
            ),
            &self.req_ctx.root_x509,
        )
        .await
    }
}
