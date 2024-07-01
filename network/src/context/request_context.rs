use async_rwlock::RwLock; // todo, consider different locks, such as the tokio implementation
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{Certificate, Client, ClientBuilder, Method, RequestBuilder};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use data::model::microservice_node_model::{MicroserviceNode, MicroserviceType};

pub trait RequestContext {
    fn client(&self) -> &Client;

    fn update_client(&mut self);
}

#[derive(Debug, Clone)]
pub struct ControllerRequestContext {
    pub node_addr: Arc<RwLock<HashMap<Uuid, String>>>,
    pub node_token: Arc<RwLock<HashMap<Uuid, String>>>,
    pub token_node: Arc<RwLock<HashMap<String, MicroserviceNode>>>,
    pub nodes: Arc<RwLock<Vec<MicroserviceNode>>>,
    pub root_certificate: Certificate,
    client: Client,
}

impl RequestContext for ControllerRequestContext {
    fn client(&self) -> &Client {
        &self.client
    }

    fn update_client(&mut self) {
        todo!()
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
            client,
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
                .request(method, url)
                .header(AUTHORIZATION, auth_token),
        )
    }
}

#[derive(Debug, Clone)]
pub struct NodeRequestContext {
    pub controller_addr: String,
    pub node_addr: Arc<RwLock<HashMap<Uuid, String>>>,
    pub access_token: String,
    pub renewal_token: String,
    pub root_certificate: Certificate,
    pub microservice_type: MicroserviceType,
    client: Client,
}

impl RequestContext for NodeRequestContext {
    fn client(&self) -> &Client {
        &self.client
    }

    fn update_client(&mut self) {
        self.client = Self::create_client(&self.access_token, &self.root_certificate)
    }
}

impl NodeRequestContext {
    pub fn new(
        controller_addr: String,
        node_addr: HashMap<Uuid, String>,
        access_token: String,
        renewal_token: String,
        root_certificate: Certificate,
        microservice_type: MicroserviceType,
    ) -> Self {
        let client = Self::create_client(&access_token, &root_certificate);
        NodeRequestContext {
            controller_addr,
            node_addr: Arc::new(RwLock::new(node_addr)),
            access_token,
            renewal_token,
            root_certificate,
            microservice_type,
            client,
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

    pub fn controller(&self, path: &str) -> String {
        format!("https://{}{path}", self.controller_addr)
    }
}
