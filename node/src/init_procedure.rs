use crate::config::node_config::NodeConfigInstance;
use commons::autoconfigure::auth_conf::{register_procedure, RegistrationResult};
use commons::context::microservice_request_context::{MicroserviceRequestContext, SecurityContext};
use data::model::microservice_node_model::MicroserviceType;
use openssl::x509::X509;
use reqwest::Certificate;
use std::collections::HashMap;
use std::fs;

pub async fn register_node(
    config: &NodeConfigInstance,
) -> (MicroserviceRequestContext, RegistrationResult) {
    let ca_cert = X509::from_pem(
        fs::read(&config.ca_certificate)
            .expect("Unable to read ca cert file")
            .as_slice(),
    )
    .expect("Invalid ca cert format");

    let security_ctx = SecurityContext {
        access_token: "".to_string(),
        renewal_token: "".to_string(),
        root_x509: ca_cert.clone(),
        root_certificate: Certificate::from_pem(ca_cert.to_pem().unwrap().as_slice())
            .expect("Invalid certificate file"),
    };

    let mut ctx = MicroserviceRequestContext::new(
        config.cnc_addr.clone(),
        HashMap::new(),
        security_ctx,
        MicroserviceType::StorageNode,
        Default::default(),
    );

    let reg_res = register_procedure(&mut ctx).await;

    (ctx, reg_res)
}
