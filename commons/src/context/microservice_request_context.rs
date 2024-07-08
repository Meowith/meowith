use crate::context::request_context::RequestContext;
use data::dto::config::PortConfiguration;
use data::dto::controller::{ValidatePeerRequest, ValidatePeerResponse};
use data::model::microservice_node_model::MicroserviceType;
use openssl::x509::X509;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{Certificate, Client, ClientBuilder};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};
use uuid::Uuid;

pub type NodeAddrMap = Arc<RwLock<HashMap<Uuid, String>>>;

#[derive(Debug, Clone)]
pub struct MicroserviceRequestContext {
    pub controller_addr: String,
    pub node_addr: NodeAddrMap,
    pub security_context: SecurityContext,
    pub microservice_type: MicroserviceType,
    pub port_configuration: PortConfiguration,
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
    ) -> Self {
        let client = Self::create_client(
            &security_context.access_token,
            &security_context.root_certificate,
        );
        MicroserviceRequestContext {
            controller_addr,
            node_addr: Arc::new(RwLock::new(node_addr)),
            security_context,
            microservice_type,
            port_configuration,
            client: Arc::new(RwLock::new(client)),
        }
    }

    fn create_client(access_token: &str, root_certificate: &Certificate) -> Client {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, access_token.parse().unwrap());
        ClientBuilder::new()
            .use_rustls_tls()
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
            .post(self.controller("/api/internal/"))
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
}
