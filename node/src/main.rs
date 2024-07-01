use crate::config::node_config::NodeConfig;
use crate::init_procedure::send_init_handshake;
use data::model::microservice_node_model::MicroserviceType;
use logging::initialize_logging;
use network::autoconfigure::auth_conf::register_procedure;
use network::context::request_context::NodeRequestContext;
use openssl::x509::X509;
use reqwest::Certificate;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

mod config;
mod init_procedure;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    initialize_logging(Some(Path::new("./log4rs.yaml")));
    let node_config: NodeConfig = NodeConfig::from_file(
        std::env::current_dir()
            .unwrap()
            .join("config.yaml")
            .to_str()
            .unwrap(),
    )
    .expect("Failed to init config");

    let config = node_config
        .validate_config()
        .expect("Failed to validate config");

    let ca_cert = X509::from_pem(
        fs::read(&config.ca_certificate)
            .expect("Unable to read ca cert file")
            .as_slice(),
    )
    .expect("Invalid ca cert format");

    let mut ctx = NodeRequestContext::new(
        config.cnc_addr.clone(),
        HashMap::new(),
        "".to_string(),
        "".to_string(),
        Certificate::from_pem(ca_cert.to_pem().unwrap().as_slice())
            .expect("Invalid certificate file"),
        MicroserviceType::StorageNode,
    );

    let _reg_res = register_procedure(&mut ctx).await;

    send_init_handshake(&config);

    Ok(())
}
