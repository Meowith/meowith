use crate::caching::mgpp_handler::{CacheInvalidationHandler, NsmData};
use commons::error::std_response::NodeClientError;
use data::dto::config::GeneralConfiguration;
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
    nsm_data: NsmData,
) -> Result<MGPPClient, NodeClientError> {
    MGPPClient::connect(
        controller_addr.to_string(),
        general_configuration.port_configuration.mgpp_server_port,
        certificate,
        microservice_id,
        Some(token),
        MGPPHandlers::new(Box::new(CacheInvalidationHandler::new(nsm_data))),
    )
    .await
    .map_err(|_| NodeClientError::InternalError)
}
