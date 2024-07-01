use crate::model::microservice_node_model::MicroserviceType;

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
}
