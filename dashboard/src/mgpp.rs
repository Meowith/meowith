use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use log::error;
use openssl::x509::X509;
use uuid::Uuid;
use commons::error::std_response::NodeClientError;
use data::dto::config::GeneralConfiguration;
use protocol::mgpp::client::MGPPClient;
use protocol::mgpp::handler::MGPPHandlers;
use crate::caching::mgpp_handler::CacheInvalidationHandler;

pub async fn connect_mgpp(
    controller_addr: &str,
    general_configuration: GeneralConfiguration,
    microservice_id: Uuid,
    certificate: X509,
    token: String,
) -> Result<MGPPClient, NodeClientError> {
    MGPPClient::connect(
        &SocketAddr::new(
            IpAddr::from_str(controller_addr).unwrap(),
            general_configuration.port_configuration.mgpp_server_port,
        ),
        microservice_id,
        certificate,
        Some(token),
        MGPPHandlers::new(Box::new(CacheInvalidationHandler::new())),
    )
    .await
    .map_err(|err| {
        error!("MGPP connect error: {err:?}");
        NodeClientError::InternalError
    })
}
