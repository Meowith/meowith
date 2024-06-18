use actix_web::{HttpRequest, post, web};
use data::model::microservice_node_model::MicroserviceType;

use crate::AppState;
use crate::discovery::discovery_service::{get_address, perform_register_node};
use crate::error::node::NodeError;

#[derive(serde::Serialize)]
pub struct NodeRegisterResponse {}

#[derive(serde::Deserialize)]
pub struct NodeRegisterRequest {
    pub code: String,
    pub service_type: MicroserviceType
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
