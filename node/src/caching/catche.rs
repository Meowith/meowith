use async_trait::async_trait;
use commons::context::microservice_request_context::{MicroserviceRequestContext, NodeStorageMap};
use data::dto::config::GeneralConfiguration;
use openssl::x509::X509;
use protocol::mgpp::client::MGPPClient;
use protocol::mgpp::handler::CatcheHandler;
use std::any::Any;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::caching::invalidator::{insert_invalidator_map, CacheInvalidator};
use commons::error::std_response::NodeClientError;

//Node storage map invalidator data type
pub type NsmData = (NodeStorageMap, Arc<MicroserviceRequestContext>);

#[derive(Debug, Default)]
pub struct CacheInvalidationHandler {
    invalidators: HashMap<u8, Box<dyn CacheInvalidator>>,
}

impl CacheInvalidationHandler {
    pub fn new(nsm_data: NsmData) -> Self {
        let mut map = HashMap::new();

        insert_invalidator_map(&mut map, nsm_data);

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
    nsm_data: NsmData,
) -> Result<MGPPClient, NodeClientError> {
    MGPPClient::connect(
        &SocketAddr::new(
            IpAddr::from_str(controller_addr).unwrap(),
            general_configuration.port_configuration.catche_server_port,
        ),
        microservice_id,
        certificate,
        Arc::new(Mutex::new(Box::new(CacheInvalidationHandler::new(
            nsm_data,
        )))),
        Some(token),
    )
    .await
    .map_err(|_| NodeClientError::InternalError)
}
