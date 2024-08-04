use crate::public::response::NodeClientError;
use async_trait::async_trait;
use data::dto::config::GeneralConfiguration;
use openssl::x509::X509;
use protocol::catche::catche_client::CatcheClient;
use protocol::catche::handler::CatcheHandler;
use std::any::Any;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug)]
pub struct CacheInvalidationHandler;

#[async_trait]
impl CatcheHandler for CacheInvalidationHandler {
    async fn handle_invalidate(&mut self, _cache_id: u32, _cache_key: String) {
        todo!();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub async fn connect_catche(
    controller_addr: &str,
    general_configuration: GeneralConfiguration,
    microservice_id: Uuid,
    certificate: X509,
    token: String,
) -> Result<CatcheClient, NodeClientError> {
    CatcheClient::connect(
        &SocketAddr::new(
            IpAddr::from_str(controller_addr).unwrap(),
            general_configuration.port_configuration.catche_server_port,
        ),
        microservice_id,
        certificate,
        Arc::new(Mutex::new(Box::new(CacheInvalidationHandler {}))),
        Some(token),
    )
    .await
    .map_err(|_| NodeClientError::InternalError)
}
