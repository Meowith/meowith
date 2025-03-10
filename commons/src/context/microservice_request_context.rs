use crate::context::request_context::RequestContext;
use data::dto::config::PortConfiguration;
use data::dto::controller::{
    UpdateStorageNodeProperties, ValidatePeerRequest, ValidatePeerResponse,
};
use data::model::microservice_node_model::MicroserviceType;
use derive_more::AsRef;
use log::trace;
use openssl::x509::X509;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{Certificate, Client, ClientBuilder};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};
use uuid::Uuid;

pub type NodeAddrMap = Arc<RwLock<HashMap<Uuid, String>>>;
pub type NodeStorageMap = Arc<RwLock<HashMap<Uuid, u64>>>;

#[derive(Debug, Clone, AsRef)]
pub struct MicroserviceRequestContext {
    pub controller_addr: String,
    pub node_addr: NodeAddrMap,
    pub security_context: SecurityContext,
    pub microservice_type: MicroserviceType,
    pub port_configuration: PortConfiguration,
    pub id: Uuid,
    pub heart_beat_interval_seconds: u64,
    client: Arc<RwLock<Client>>,
}

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub access_token: String,
    pub renewal_token: String,
    pub root_certificate: Certificate,
    pub root_x509: X509,
}

#[derive(Debug, Clone, derive_more::Display)]
pub enum AddressError {
    InvalidNodeId,
}

#[derive(Debug, Clone, derive_more::Display)]
pub enum HeartBeatError {
    BadRequest(String),
}

impl Error for HeartBeatError {}

impl Error for AddressError {}

impl RequestContext for MicroserviceRequestContext {
    async fn client(&self) -> RwLockReadGuard<Client> {
        self.client.read().await
    }

    fn update_client(&mut self) {
        self.client = Arc::new(RwLock::new(Self::create_client(
            &self.security_context.access_token,
            &self.security_context.root_certificate,
        )))
    }
}

impl MicroserviceRequestContext {
    pub fn new(
        controller_addr: String,
        node_addr: HashMap<Uuid, String>,
        security_context: SecurityContext,
        microservice_type: MicroserviceType,
        port_configuration: PortConfiguration,
        heart_beat_interval_seconds: u64,
        id: Uuid,
    ) -> Self {
        let client = Self::create_client(
            &security_context.access_token,
            &security_context.root_certificate,
        );
        MicroserviceRequestContext {
            controller_addr,
            node_addr: Arc::new(RwLock::new(node_addr)),
            security_context,
            heart_beat_interval_seconds,
            microservice_type,
            port_configuration,
            id,
            client: Arc::new(RwLock::new(client)),
        }
    }

    fn create_client(access_token: &str, root_certificate: &Certificate) -> Client {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, access_token.parse().unwrap());
        ClientBuilder::new()
            .add_root_certificate(root_certificate.clone())
            .default_headers(headers)
            .build()
            .unwrap()
    }

    pub async fn validate_peer_token(
        &self,
        peer_token: String,
        id: Uuid,
    ) -> Result<ValidatePeerResponse, Box<dyn Error>> {
        let resp = self
            .client()
            .await
            .post(self.controller("/api/internal/validate/peer"))
            .json(&ValidatePeerRequest {
                node_token: peer_token,
                node_id: id,
            })
            .send()
            .await?
            .json::<ValidatePeerResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn heartbeat(&self) -> Result<(), Box<dyn Error>> {
        trace!("Performing a heartbeat");
        let response = self
            .client()
            .await
            .post(self.controller("/api/internal/health/heartbeat"))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Box::new(HeartBeatError::BadRequest(format!(
                "Received a non 2xx response: [{:?}]: {:?}",
                response.status(),
                response.text().await
            ))));
        }
        Ok(())
    }

    pub async fn update_storage(
        &self,
        req: UpdateStorageNodeProperties,
    ) -> Result<(), Box<dyn Error>> {
        trace!("Performing a health storage update");
        let response = self
            .client()
            .await
            .post(self.controller("/api/internal/health/storage"))
            .json(&req)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Box::new(HeartBeatError::BadRequest(format!(
                "Received a non 2xx response: [{:?}]: {:?}",
                response.status(),
                response.text().await
            ))));
        }
        Ok(())
    }

    pub fn controller(&self, path: &str) -> String {
        format!("https://{}{path}", self.controller_addr)
    }

    pub async fn node_internal(&self, node: &Uuid, path: &str) -> Result<String, AddressError> {
        Ok(format!(
            "https://{}:{}{path}",
            self.node_addr
                .read()
                .await
                .get(node)
                .ok_or(AddressError::InvalidNodeId)?,
            self.port_configuration.internal_server_port
        ))
    }

    pub async fn shutdown(&self) {
        // Drop the old client, closing its pool.
        *self.client.write().await = Client::new();
    }
}
