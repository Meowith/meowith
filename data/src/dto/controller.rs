use crate::model::microservice_node_model::MicroserviceType;
use std::collections::HashMap;
use uuid::Uuid;

pub static X_ADDR_HEADER: &str = "X-Custom-Addr";

#[derive(serde::Deserialize, serde::Serialize)]
pub struct NodeRegisterRequest {
    pub code: String,
    pub service_type: MicroserviceType,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct NodeRegisterResponse {
    pub renewal_token: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct AuthenticationRequest {
    pub renewal_token: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct AuthenticationResponse {
    pub access_token: String,
    pub id: Uuid,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ValidatePeerRequest {
    pub node_token: String,
    pub node_id: Uuid,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ValidatePeerResponse {
    pub valid: bool,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct StorageResponse {
    pub peers: HashMap<Uuid, u64>,
}
