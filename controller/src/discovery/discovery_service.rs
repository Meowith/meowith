use std::net::IpAddr;

use actix_web::http::header::HeaderValue;
use actix_web::web::Bytes;
use actix_web::{web, HttpRequest};
use chrono::prelude::*;
use futures_util::TryFutureExt;
use openssl::x509::X509Req;
use scylla::CachingSession;
use uuid::Uuid;

use commons::autoconfigure::ssl_conf::SigningData;
use commons::context::controller_request_context::ControllerRequestContext;
use data::access::microservice_node_access::{
    get_service_register_code, insert_microservice_node, update_service_access_token,
    update_service_register_code,
};
use data::dto::controller::{
    AuthenticationRequest, AuthenticationResponse, NodeRegisterRequest, NodeRegisterResponse,
};
use data::error::MeowithDataError;
use data::model::microservice_node_model::MicroserviceNode;

use crate::error::node::NodeError;
use crate::token_service::{generate_access_token, generate_renewal_token};
use crate::AppState;

pub async fn perform_register_node(
    req: NodeRegisterRequest,
    ctx: &ControllerRequestContext,
    session: &CachingSession,
    node_addr: IpAddr,
) -> Result<NodeRegisterResponse, NodeError> {
    let code = get_service_register_code(req.code, session).await;
    if let Err(err) = code {
        return match err {
            MeowithDataError::NotFound => Err(NodeError::BadRequest),
            _ => Err(NodeError::InternalError),
        };
    }
    let mut code = code.unwrap();
    if !code.valid {
        return Err(NodeError::BadRequest);
    }

    let token = generate_renewal_token().to_string();

    let service = MicroserviceNode {
        microservice_type: req.service_type,
        id: Uuid::new_v4(),
        max_space: None,
        used_space: None,
        address: node_addr,
        created: Utc::now(),
        register_code: code.code.clone(),
        renewal_token: token.clone(),
        access_token: None,
        access_token_issued_at: DateTime::from_timestamp_millis(0).unwrap(),
    };

    code.valid = false;
    update_service_register_code(code, session)
        .await
        .map_err(|_| NodeError::InternalError)?;
    insert_microservice_node(service.clone(), session)
        .await
        .map_err(|_| NodeError::InternalError)?;

    {
        // Note. We aren't inserting into the token map, as the access_token doesn't yet exist.
        let mut nodes = ctx.nodes.write().await;
        nodes.push(service);
    }

    Ok(NodeRegisterResponse {
        renewal_token: token,
    })
}

pub async fn perform_token_creation(
    state: web::Data<AppState>,
    req: AuthenticationRequest,
) -> Result<AuthenticationResponse, NodeError> {
    let mut nodes = state.req_ctx.nodes.write().await;
    let authorized_node = nodes
        .iter_mut()
        .find(|n| n.renewal_token == req.renewal_token);

    if let Some(node) = authorized_node {
        let access_token = generate_access_token();
        update_service_access_token(node, &state.session, Utc::now())
            .map_err(|_| NodeError::InternalError)
            .await?;
        let old_token = node.access_token.clone();
        node.access_token = Some(access_token.clone());

        // Update quick lookup token map
        let mut node_tk_map = state.req_ctx.token_node.write().await;
        if let Some(token) = old_token {
            node_tk_map.remove(&token);
        }
        node_tk_map.insert(access_token.clone(), node.clone());

        Ok(AuthenticationResponse {
            access_token,
            id: node.id,
        })
    } else {
        Err(NodeError::BadAuth)
    }
}

pub async fn sign_node_csr(
    renewal_token: Option<&HeaderValue>,
    node: MicroserviceNode,
    csr: X509Req,
    ip_addr: IpAddr,
    state: web::Data<AppState>,
) -> Result<Bytes, NodeError> {
    if renewal_token.is_none()
        || node.renewal_token
            != renewal_token
                .unwrap()
                .to_str()
                .map_err(|_| NodeError::BadRequest)?
    {
        return Err(NodeError::BadAuth);
    }

    let signing_data = SigningData {
        ip_addr,
        validity_days: state.config.autogen_ssl_validity,
    };
    let cert = commons::autoconfigure::ssl_conf::sign_csr(
        &csr,
        &state.ca_cert,
        &state.ca_private_key,
        &signing_data,
    )
    .map_err(|_| NodeError::InternalError)?;
    Ok(Bytes::from(
        cert.to_der().map_err(|_| NodeError::InternalError)?,
    ))
}

// Note: Its worth considering a self-reported address as it allows for potential proxy usage
pub fn get_address(req: &HttpRequest) -> Result<IpAddr, ()> {
    if let Some(sock_addr) = req.peer_addr() {
        Ok(sock_addr.ip())
    } else {
        Err(())
    }
}
