use actix_web::http::header::HeaderValue;
use actix_web::web::Bytes;
use actix_web::{web, HttpRequest};
use chrono::prelude::*;
use commons::autoconfigure::addr_header::deserialize_header;
use futures_util::TryFutureExt;
use log::{error, info, warn};
use openssl::x509::X509Req;
use scylla::client::caching_session::CachingSession;
use std::net::IpAddr;
use std::str::FromStr;
use uuid::Uuid;

use crate::error::node::NodeError;
use crate::token_service::{generate_access_token, generate_renewal_token};
use crate::AppState;
use commons::autoconfigure::ssl_conf::SigningData;
use commons::cache::CacheId;
use commons::context::controller_request_context::ControllerRequestContext;
use data::access::microservice_node_access::{
    get_service_register_code, insert_microservice_node, update_service_access_token,
    update_service_register_code,
};
use data::dto::controller::{
    AuthenticationRequest, AuthenticationResponse, NodeRegisterRequest, NodeRegisterResponse,
    X_ADDR_HEADER,
};
use data::error::MeowithDataError;
use data::model::microservice_node_model::MicroserviceNode;
use protocol::mgpp::packet::MGPPPacket;

pub async fn perform_register_node(
    req: NodeRegisterRequest,
    ctx: &ControllerRequestContext,
    session: &CachingSession,
    node_addr: IpAddr,
) -> Result<NodeRegisterResponse, NodeError> {
    info!(
        "Attempting node registration. addr={node_addr} code='{}'",
        req.code
    );
    let code = get_service_register_code(req.code, session).await;
    if let Err(err) = code {
        warn!("Node registration failed, code not found {err}");
        return match err {
            MeowithDataError::NotFound => Err(NodeError::BadRequest),
            _ => Err(NodeError::InternalError),
        };
    }
    let mut code = code.unwrap();
    if !code.valid {
        warn!("Node registration failed, code not valid");
        return Err(NodeError::BadRequest);
    }

    let token = generate_renewal_token().to_string();

    let service = MicroserviceNode {
        microservice_type: req.service_type.into(),
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

    info!("Node registration successful");

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
        let old_token = node.access_token.clone();
        node.access_token = Some(access_token.clone());
        update_service_access_token(node, &state.session, Utc::now())
            .map_err(|_| NodeError::InternalError)
            .await?;

        // Update quick lookup token map
        let mut node_tk_map = state.req_ctx.token_node.write().await;
        let mut tk_node_map = state.req_ctx.node_token.write().await;
        if let Some(token) = old_token {
            node_tk_map.remove(&token);
        }
        tk_node_map.insert(node.id, access_token.clone());
        node_tk_map.insert(access_token.clone(), node.clone());

        let cache_id: u8 = CacheId::NodeStorageMap.into();
        if let Err(e) = state
            .mgpp_server
            .broadcast_packet(MGPPPacket::InvalidateCache {
                cache_id: cache_id as u32,
                cache_key: vec![],
            })
            .await
        {
            error!("MGPP Failed to broadcast packet during token creation {e}");
        }

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
    ip_addrs: Vec<IpAddr>,
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
        ip_addrs,
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
// Note: considered it.
pub fn get_address(req: &HttpRequest) -> Result<IpAddr, ()> {
    IpAddr::from_str(
        req.headers()
            .get(X_ADDR_HEADER)
            .ok_or(())?
            .to_str()
            .map_err(|_| ())?,
    )
    .map_err(|_| ())
}
pub fn get_addresses(req: &HttpRequest) -> Result<Vec<IpAddr>, ()> {
    let header = req
        .headers()
        .get(X_ADDR_HEADER)
        .ok_or(())?
        .to_str()
        .map_err(|_| ())?;

    deserialize_header(header.to_string()).map_err(|_| ())
}
