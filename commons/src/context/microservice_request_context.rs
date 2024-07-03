use crate::context::request_context::RequestContext;
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

#[derive(Debug, Clone)]
pub struct MicroserviceRequestContext {
    pub controller_addr: String,
    pub node_addr: Arc<RwLock<HashMap<Uuid, String>>>,
    pub access_token: String,
    pub renewal_token: String,
    pub root_certificate: Certificate,
    pub root_x509: X509,
    pub microservice_type: MicroserviceType,
    client: Arc<RwLock<Client>>,
}

impl RequestContext for MicroserviceRequestContext {
    async fn client(&self) -> RwLockReadGuard<Client> {
        self.client.read().await
    }

    fn update_client(&mut self) {
        self.client = Arc::new(RwLock::new(Self::create_client(
            &self.access_token,
            &self.root_certificate,
        )))
    }
}

impl MicroserviceRequestContext {
    pub fn new(
        controller_addr: String,
        node_addr: HashMap<Uuid, String>,
        access_token: String,
        renewal_token: String,
        root_x509: X509,
        root_certificate: Certificate,
        microservice_type: MicroserviceType,
    ) -> Self {
        let client = Self::create_client(&access_token, &root_certificate);
        MicroserviceRequestContext {
            controller_addr,
            node_addr: Arc::new(RwLock::new(node_addr)),
            access_token,
            renewal_token,
            root_certificate,
            root_x509,
            microservice_type,
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
}
