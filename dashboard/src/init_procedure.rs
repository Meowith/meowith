use crate::dashboard_config::DashboardConfig;
use commons::autoconfigure::auth_conf::{register_procedure, RegistrationResult};
use commons::context::microservice_request_context::{MicroserviceRequestContext, SecurityContext};
use data::model::microservice_node_model::MicroserviceType;
use logging::log_err;
use openssl::x509::X509;
use reqwest::Certificate;
use std::fs;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::AbortHandle;
use tokio::time;
use uuid::Uuid;

pub async fn register_node(
    config: &DashboardConfig,
) -> (MicroserviceRequestContext, RegistrationResult) {
    let ca_cert = X509::from_pem(
        fs::read(&config.ca_certificate)
            .unwrap_or_else(|_| panic!("Unable to read ca cert file {}", &config.ca_certificate))
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
        format!("{}:{}", config.cnc_addr.clone(), config.cnc_port.clone()),
        std::collections::HashMap::new(),
        security_ctx,
        MicroserviceType::Dashboard,
        Default::default(),
        config.heart_beat_interval_seconds,
        Uuid::new_v4(),
    );

    let reg_res = register_procedure(
        &mut ctx,
        config.broadcast_address.clone(),
        config
            .cert_addresses
            .iter()
            .map(|addr| {
                IpAddr::from_str(addr.as_str()).expect("Invalid certificate signing addresses")
            })
            .collect(),
        config.cert_domains.clone(),
        config.renewal_token_path.clone(),
    )
    .await;

    (ctx, reg_res)
}

pub fn initializer_heart(req_ctx: Arc<MicroserviceRequestContext>) -> AbortHandle {
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(req_ctx.heart_beat_interval_seconds));
        loop {
            interval.tick().await;
            let res = req_ctx.heartbeat().await;
            log_err("Heartbeat err", res);
        }
    })
    .abort_handle()
}
