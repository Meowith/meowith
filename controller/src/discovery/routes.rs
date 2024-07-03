use crate::discovery::discovery_service::{
    get_address, perform_register_node, perform_storage_node_properties_update,
    perform_token_creation,
};
use crate::error::node::NodeError;
use crate::AppState;
use actix_web::web::Bytes;
use actix_web::{post, web, HttpRequest};
use commons::autoconfigure::ssl_conf::{sign_csr, SigningData};
use data::dto::controller::{
    AuthenticationRequest, AuthenticationResponse, NodeRegisterRequest, NodeRegisterResponse,
    ValidatePeerRequest, ValidatePeerResponse,
};
use data::model::microservice_node_model::MicroserviceNode;
use openssl::x509::X509Req;

#[derive(serde::Deserialize)]
pub struct UpdateStorageNodeProperties {
    pub max_space: u64,
    pub used_space: u64,
}

#[derive(serde::Serialize)]
pub struct UpdateStorageNodeResponse {}

#[post("/security/csr")]
pub async fn security_csr(
    state: web::Data<AppState>,
    body: Bytes,
    node: MicroserviceNode,
    http_request: HttpRequest,
) -> Result<Bytes, NodeError> {
    // TODO, move to service
    let renewal_token = http_request.headers().get("Sec-Authorization");
    if renewal_token.is_none()
        || node.renewal_token
            != renewal_token
                .unwrap()
                .to_str()
                .map_err(|_| NodeError::BadRequest)?
    {
        return Err(NodeError::BadAuth);
    }

    let csr = X509Req::from_der(body.as_ref()).map_err(|_| NodeError::BadRequest)?;
    let signing_data = SigningData {
        ip_addr: get_address(&http_request).map_err(|_| NodeError::BadRequest)?,
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
    Ok(web::Json(
        perform_register_node(
            req.0,
            &state.req_ctx,
            &state.session,
            get_address(&http_request).map_err(|_| NodeError::BadRequest)?,
        )
        .await?,
    ))
}

#[post("/validate/peer")]
pub async fn validate_peer(
    state: web::Data<AppState>,
    req: web::Json<ValidatePeerRequest>,
) -> Result<web::Json<ValidatePeerResponse>, NodeError> {
    let map = state.req_ctx.node_token.read().await;

    if let Some(token) = map.get(&req.0.node_id) {
        Ok(web::Json(ValidatePeerResponse {
            valid: *token == req.node_token,
        }))
    } else {
        Ok(web::Json(ValidatePeerResponse { valid: false }))
    }
}

#[post("/authenticate")]
pub async fn authenticate_node(
    state: web::Data<AppState>,
    req: web::Json<AuthenticationRequest>,
) -> Result<web::Json<AuthenticationResponse>, NodeError> {
    Ok(web::Json(perform_token_creation(state, req.0).await?))
}

#[post("/storage")]
pub async fn update_storage_node_properties(
    state: web::Data<AppState>,
    node: MicroserviceNode,
    req: web::Json<UpdateStorageNodeProperties>,
) -> Result<web::Json<UpdateStorageNodeResponse>, NodeError> {
    perform_storage_node_properties_update(req.0, &state.session, node).await?;

    Ok(web::Json(UpdateStorageNodeResponse {}))
}
