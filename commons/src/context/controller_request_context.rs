use crate::context::request_context::RequestContext;
use data::model::microservice_node_model::MicroserviceNode;
use reqwest::header::AUTHORIZATION;
use reqwest::{Certificate, Client, ClientBuilder, Method, RequestBuilder};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ControllerRequestContext {
    pub node_addr: Arc<RwLock<HashMap<Uuid, String>>>,
    pub node_token: Arc<RwLock<HashMap<Uuid, String>>>,
    pub token_node: Arc<RwLock<HashMap<String, MicroserviceNode>>>,
    pub nodes: Arc<RwLock<Vec<MicroserviceNode>>>,
    pub root_certificate: Certificate,
    client: Arc<RwLock<Client>>,
}

impl RequestContext for ControllerRequestContext {
    async fn client(&self) -> RwLockReadGuard<Client> {
        self.client.read().await
    }

    fn update_client(&mut self) {
        // Noop, as this method is never actually called.
    }
}

impl ControllerRequestContext {
    pub fn new(
        node_addr: HashMap<Uuid, String>,
        node_token: HashMap<Uuid, String>,
        token_node: HashMap<String, MicroserviceNode>,
        nodes: Vec<MicroserviceNode>,
        root_certificate: Certificate,
    ) -> Self {
        let client = ClientBuilder::new()
            .use_rustls_tls()
            .add_root_certificate(root_certificate.clone())
            .build()
            .unwrap();

        ControllerRequestContext {
            node_addr: Arc::new(RwLock::new(node_addr)),
            node_token: Arc::new(RwLock::new(node_token)),
            token_node: Arc::new(RwLock::new(token_node)),
            nodes: Arc::new(RwLock::new(nodes)),
            root_certificate,
            client: Arc::new(RwLock::new(client)),
        }
    }

    /// Prepopulates the authorization for the requested node.
    pub async fn request_for(
        &self,
        node_id: &Uuid,
        method: Method,
        url: &String,
    ) -> Option<RequestBuilder> {
        let nt_map = self.node_token.read().await;
        let auth_token = nt_map.get(node_id)?;
        let client = self.client();
        Some(
            client
                .await
                .request(method, url)
                .header(AUTHORIZATION, auth_token),
        )
    }
}
