use actix_web::{HttpRequest, post, web};
use data::model::microservice_node_model::MicroserviceType;

use crate::AppState;
use crate::discovery::discovery_service::{get_address, perform_register_node, perform_storage_node_properties_update};
use crate::error::node::NodeError;

#[derive(serde::Serialize)]
pub struct NodeRegisterResponse {}

#[derive(serde::Deserialize)]
pub struct NodeRegisterRequest {
    pub code: String,
    pub service_type: MicroserviceType
}

#[derive(serde::Deserialize)]
pub struct UpdateStorageNodeProperties {
    pub max_space: u64,
    pub used_space: u64
}

#[post("/register")]
pub async fn register_node(
    state: web::Data<AppState>,
    req: web::Json<NodeRegisterRequest>,
    http_request: HttpRequest
) -> Result<web::Json<NodeRegisterResponse>, NodeError> {
    perform_register_node(req.0, &state.session, get_address(&http_request)?).await?;

    Ok(web::Json(NodeRegisterResponse {}))
}

#[post("/storage")]
pub async fn update_storage_node_properties(
    state: web::Data<AppState>,
    req: web::Json<UpdateStorageNodeProperties>,
    http_request: HttpRequest
) -> Result<web::Json<NodeRegisterResponse>, NodeError> {
    perform_storage_node_properties_update(req.0, &state.session, get_address(&http_request)?).await?;

    Ok(web::Json(NodeRegisterResponse {}))
}
