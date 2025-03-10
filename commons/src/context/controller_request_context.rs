use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use data::dto::controller::UpdateStorageNodeProperties;
use data::model::microservice_node_model::MicroserviceNode;
use reqwest::header::AUTHORIZATION;
use reqwest::{Certificate, Client, ClientBuilder, Method, RequestBuilder};
use tokio::sync::{RwLock, RwLockReadGuard};
use uuid::Uuid;

use crate::context::request_context::RequestContext;

#[derive(Debug, Clone)]
pub struct ControllerRequestContext {
    pub node_addr: Arc<RwLock<HashMap<Uuid, String>>>,
    pub node_token: Arc<RwLock<HashMap<Uuid, String>>>,
    pub token_node: Arc<RwLock<HashMap<String, MicroserviceNode>>>,
    pub node_health: Arc<RwLock<HashMap<Uuid, NodeHealth>>>,
    pub nodes: Arc<RwLock<Vec<MicroserviceNode>>>,
    pub root_certificate: Certificate,
    client: Arc<RwLock<Client>>,
}

#[derive(Debug, Clone)]
pub struct NodeHealth {
    pub last_beat: DateTime<Utc>,
    pub info: Option<UpdateStorageNodeProperties>,
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
            .add_root_certificate(root_certificate.clone())
            .build()
            .unwrap();

        ControllerRequestContext {
            node_addr: Arc::new(RwLock::new(node_addr)),
            node_token: Arc::new(RwLock::new(node_token)),
            token_node: Arc::new(RwLock::new(token_node)),
            node_health: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
            root_certificate,
            client: Arc::new(RwLock::new(client)),
        }
    }

    pub async fn remove_node_from_maps(&self, id: Uuid) {
        let mut nodes = self.nodes.write().await;
        nodes.retain(|x| x.id != id);
        let mut nodes = self.node_health.write().await;
        nodes.remove(&id);
        let mut nodes = self.node_token.write().await;
        let token = nodes.remove(&id);
        let mut nodes = self.node_addr.write().await;
        nodes.remove(&id);
        if let Some(token) = token {
            let mut nodes = self.token_node.write().await;
            nodes.remove(&token);
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
