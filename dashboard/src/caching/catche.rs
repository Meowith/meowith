use std::any::Any;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use log::error;
use openssl::x509::X509;
use tokio::sync::Mutex;
use uuid::Uuid;

use data::dto::config::GeneralConfiguration;
use protocol::catche::catche_client::CatcheClient;
use protocol::catche::handler::CatcheHandler;

use crate::caching::invalidator::{insert_invalidator_map, CacheInvalidator};
use commons::error::std_response::NodeClientError;

#[derive(Debug, Default)]
pub struct CacheInvalidationHandler {
    invalidators: HashMap<u8, Box<dyn CacheInvalidator>>,
}

impl CacheInvalidationHandler {
    pub fn new() -> Self {
        let mut map = HashMap::new();

        insert_invalidator_map(&mut map);

        CacheInvalidationHandler { invalidators: map }
    }
}

#[async_trait]
impl CatcheHandler for CacheInvalidationHandler {
    async fn handle_invalidate(&mut self, cache_id: u32, cache_key: &[u8]) {
        let invalidator = self.invalidators.get(&(cache_id as u8));
        if let Some(invalidator) = invalidator {
            invalidator.invalidate(cache_key).await;
        }
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
        Arc::new(Mutex::new(Box::new(CacheInvalidationHandler::new()))),
        Some(token),
    )
    .await
    .map_err(|err| {
        error!("Catche connect error: {err:?}");
        NodeClientError::InternalError
    })
}
