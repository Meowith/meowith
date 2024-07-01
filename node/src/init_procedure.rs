use crate::config::node_config::NodeConfigInstance;
use data::model::microservice_node_model::MicroserviceType;
use network::autoconfigure::auth_conf::register_procedure;
use network::context::microservice_request_context::MicroserviceRequestContext;
use openssl::x509::X509;
use reqwest::Certificate;
use std::collections::HashMap;
use std::fs;

pub async fn register_node(config: &NodeConfigInstance) {
    let ca_cert = X509::from_pem(
        fs::read(&config.ca_certificate)
            .expect("Unable to read ca cert file")
            .as_slice(),
    )
    .expect("Invalid ca cert format");

    let mut ctx = MicroserviceRequestContext::new(
        config.cnc_addr.clone(),
        HashMap::new(),
        "".to_string(),
        "".to_string(),
        ca_cert.clone(),
        Certificate::from_pem(ca_cert.to_pem().unwrap().as_slice())
            .expect("Invalid certificate file"),
        MicroserviceType::StorageNode,
    );

    let _ = register_procedure(&mut ctx).await;
}
