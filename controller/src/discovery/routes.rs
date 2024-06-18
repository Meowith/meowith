use actix_web::{post, web};
use crate::AppState;
use crate::discovery::discovery_service::perform_register_node;
use crate::error::node::NodeError;


#[derive(serde::Serialize)]
pub struct NodeRegisterResponse {
}

#[derive(serde::Deserialize)]
pub struct NodeRegisterRequest {
}

#[post("/register")]
pub async fn register_node(
    state: web::Data<AppState>,
    req: web::Json<NodeRegisterRequest>
) -> Result<web::Json<NodeRegisterResponse>, NodeError> {
    perform_register_node(req.0, &state.session).await?;

    Ok(web::Json(NodeRegisterResponse { }))
}