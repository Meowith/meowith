use data::model::microservice_node_model::MicroserviceNode;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::{Certificate, Client, ClientBuilder};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct NodeRequestContext {
    pub controller_addr: String,
    pub node_addr: HashMap<Uuid, String>,
    pub auth_token: String,
    pub root_certificate: Certificate,
}

#[derive(Debug, Clone)]
pub struct ControllerRequestContext {
    pub node_addr: HashMap<Uuid, String>,
    pub node_token: HashMap<Uuid, String>,
    pub token_node: HashMap<String, MicroserviceNode>,
    pub root_certificate: Certificate,
}

pub trait RequestContext {
    fn client(&self) -> Client;
}

impl RequestContext for NodeRequestContext {
    fn client(&self) -> Client {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, self.auth_token.parse().unwrap());
        ClientBuilder::new()
            .use_rustls_tls()
            .add_root_certificate(self.root_certificate.clone())
            .default_headers(headers)
            .build()
            .unwrap()
    }
}

impl NodeRequestContext {
    pub fn controller(&self, path: &str) -> String {
        format!("https://{}{path}", self.controller_addr)
    }
}
