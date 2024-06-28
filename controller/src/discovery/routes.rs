use actix_web::{post, web, HttpRequest};
use actix_web::web::Bytes;
use data::model::microservice_node_model::MicroserviceType;
use openssl::x509::X509Req;

use crate::discovery::discovery_service::{
    get_address, perform_register_node, perform_storage_node_properties_update,
};
use crate::error::node::NodeError;
use crate::AppState;
use network::autoconfigure::ssl_conf::{sign_csr, SigningData};

#[derive(serde::Serialize)]
pub struct NodeRegisterResponse {}

#[derive(serde::Deserialize)]
pub struct NodeRegisterRequest {
    pub code: String,
    pub service_type: MicroserviceType,
}

#[derive(serde::Deserialize)]
pub struct UpdateStorageNodeProperties {
    pub max_space: u64,
    pub used_space: u64,
}

#[post("/security/csr")]
pub async fn security_csr(
    state: web::Data<AppState>,
    body: Bytes,
    http_request: HttpRequest,
) -> Result<Bytes, NodeError> {
    let csr = X509Req::from_der(body.as_ref()).map_err(|_| NodeError::BadRequest)?;
    let signing_data = SigningData {
        ip_addr: get_address(&http_request)?,
        validity_days: state.config.autogen_ssl_validity,
    };
    let cert = sign_csr(&csr, &state.ca_cert, &state.ca_private_key, &signing_data)
        .map_err(|_| NodeError::InternalError)?;
    Ok(Bytes::from(
        cert.to_der().map_err(|_| NodeError::InternalError)?,
    ))
}

#[post("/register")]
pub async fn register_node(
    state: web::Data<AppState>,
    req: web::Json<NodeRegisterRequest>,
    http_request: HttpRequest,
) -> Result<web::Json<NodeRegisterResponse>, NodeError> {
    perform_register_node(req.0, &state.session, get_address(&http_request)?).await?;

    Ok(web::Json(NodeRegisterResponse {}))
}

#[post("/storage")]
pub async fn update_storage_node_properties(
    state: web::Data<AppState>,
    req: web::Json<UpdateStorageNodeProperties>,
    http_request: HttpRequest,
) -> Result<web::Json<NodeRegisterResponse>, NodeError> {
    perform_storage_node_properties_update(req.0, &state.session, get_address(&http_request)?)
        .await?;

    Ok(web::Json(NodeRegisterResponse {}))
}
