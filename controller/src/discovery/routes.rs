use actix_web::web::Bytes;
use actix_web::{get, post, web, HttpRequest};
use openssl::x509::X509Req;

use data::dto::config::GeneralConfiguration;
use data::dto::controller::{
    AuthenticationRequest, AuthenticationResponse, NodeRegisterRequest, NodeRegisterResponse,
    ValidatePeerRequest, ValidatePeerResponse,
};
use data::model::microservice_node_model::MicroserviceNode;

use crate::discovery::discovery_service::{
    get_address, get_addresses, perform_register_node, perform_token_creation, sign_node_csr,
};
use crate::error::node::NodeError;
use crate::AppState;

#[derive(serde::Serialize)]
pub struct UpdateStorageNodeResponse {}

#[post("/security/csr")]
pub async fn security_csr(
    state: web::Data<AppState>,
    body: Bytes,
    node: MicroserviceNode,
    http_request: HttpRequest,
) -> Result<Bytes, NodeError> {
    let renewal_token = http_request.headers().get("Sec-Authorization");
    let csr = X509Req::from_der(body.as_ref()).map_err(|_| NodeError::BadRequest)?;
    let ip_addrs = get_addresses(&http_request).map_err(|_| NodeError::BadRequest)?;
    sign_node_csr(renewal_token, node, csr, ip_addrs, state).await
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
    _node: MicroserviceNode,
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

#[get("/autoconfigure/config")]
pub async fn config_fetch(
    state: web::Data<AppState>,
    _node: MicroserviceNode,
) -> Result<web::Json<GeneralConfiguration>, NodeError> {
    let gen_cfg = state.config.general_configuration.clone();
    Ok(web::Json(gen_cfg))
}

#[post("/authenticate")]
pub async fn authenticate_node(
    state: web::Data<AppState>,
    req: web::Json<AuthenticationRequest>,
) -> Result<web::Json<AuthenticationResponse>, NodeError> {
    Ok(web::Json(perform_token_creation(state, req.0).await?))
}
