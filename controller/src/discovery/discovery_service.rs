use actix_web::HttpRequest;
use std::net::IpAddr;

use chrono::prelude::*;
use scylla::CachingSession;
use uuid::Uuid;

use data::access::microservice_node_access::{
    get_service_register_code, insert_microservice_node, update_service_register_code,
};
use data::error::MeowithDataError;
use data::model::microservice_node_model::MicroserviceNode;

use crate::discovery::routes::{
    NodeRegisterRequest, NodeRegisterResponse, UpdateStorageNodeProperties,
};
use crate::error::node::NodeError;
use crate::token_service::generate_renewal_token;

pub async fn perform_register_node(
    req: NodeRegisterRequest,
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
        token: token.clone(),
    };

    code.valid = false;
    update_service_register_code(code, session)
        .await
        .map_err(|_| NodeError::InternalError)?;
    insert_microservice_node(service, session)
        .await
        .map_err(|_| NodeError::InternalError)?;

    Ok(NodeRegisterResponse { token })
}

#[allow(unused)]
pub async fn perform_storage_node_properties_update(
    req: UpdateStorageNodeProperties,
    session: &CachingSession,
    node: MicroserviceNode,
) -> Result<(), NodeError> {
    Ok(())
}

// Note: Its worth considering a self-reported address as it allows for potential proxy usage
pub fn get_address(req: &HttpRequest) -> Result<IpAddr, ()> {
    if let Some(sock_addr) = req.peer_addr() {
        Ok(sock_addr.ip())
    } else {
        Err(())
    }
}
