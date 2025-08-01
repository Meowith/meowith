use crate::caching::mgpp_handler::CacheInvalidationHandler;
use commons::error::std_response::NodeClientError;
use data::dto::config::GeneralConfiguration;
use log::error;
use openssl::x509::X509;
use protocol::mgpp::client::MGPPClient;
use protocol::mgpp::handler::MGPPHandlers;
use uuid::Uuid;

pub async fn connect_mgpp(
    controller_addr: &str,
    general_configuration: GeneralConfiguration,
    microservice_id: Uuid,
    certificate: X509,
    token: String,
) -> Result<MGPPClient, NodeClientError> {
    MGPPClient::connect(
        controller_addr.to_string(),
        general_configuration.port_configuration.mgpp_server_port,
        certificate,
        microservice_id,
        Some(token),
        MGPPHandlers::new(Box::new(CacheInvalidationHandler::new())),
    )
    .await
    .map_err(|err| {
        error!("MGPP connect error: {err:?}");
        NodeClientError::InternalError
    })
}
