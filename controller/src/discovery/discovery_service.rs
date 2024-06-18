use std::net::IpAddr;
use actix_web::HttpRequest;

use chrono::prelude::*;
use scylla::CachingSession;
use uuid::Uuid;

use data::access::microservice_node_access::{get_service_register_code, insert_microservice_node, update_service_register_code};
use data::error::MeowithDataError;
use data::model::microservice_node_model::MicroserviceNode;

use crate::discovery::routes::NodeRegisterRequest;
use crate::error::node::NodeError;

pub async fn perform_register_node(
    req: NodeRegisterRequest,
    session: &CachingSession,
    node_addr: IpAddr
) -> Result<(), NodeError> {
    let code = get_service_register_code(req.code, session).await;
    if let Err(err) = code {
        return match err {
            MeowithDataError::NotFound => Err(NodeError::BadRequest),
            _ => Err(NodeError::InternalError)
        };
    }
    let mut code = code.unwrap();
    if !code.valid { return Err(NodeError::BadRequest); }

    let service = MicroserviceNode {
        microservice_type: req.service_type,
        id: Uuid::new_v4(),
        max_space: None,
        used_space: None,
        address: node_addr,
        created: Utc::now(),
        register_code: "".to_string(),
    };

    code.valid = false;
    update_service_register_code(code, session).await.map_err(|_| NodeError::InternalError)?;
    insert_microservice_node(service, session).await.map_err(|_| NodeError::InternalError)?;

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