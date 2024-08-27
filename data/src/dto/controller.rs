use crate::model::microservice_node_model::MicroserviceType;
use std::collections::HashMap;
use std::net::IpAddr;
use uuid::Uuid;

pub static X_ADDR_HEADER: &str = "X-Custom-Addr";

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct NodeRegisterRequest {
    pub code: String,
    pub service_type: MicroserviceType,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct NodeRegisterResponse {
    pub renewal_token: String,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct AuthenticationRequest {
    pub renewal_token: String,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct AuthenticationResponse {
    pub access_token: String,
    pub id: Uuid,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct ValidatePeerRequest {
    pub node_token: String,
    pub node_id: Uuid,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct ValidatePeerResponse {
    pub valid: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct PeerStorage {
    pub storage: u64,
    pub addr: IpAddr,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct StorageResponse {
    pub peers: HashMap<Uuid, PeerStorage>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct UpdateStorageNodeProperties {
    pub max_space: u64,
    pub used_space: u64,
}
