use crate::discovery::routes::{UpdateStorageNodeProperties, UpdateStorageNodeResponse};
use crate::error::node::NodeError;
use crate::health::health_service::perform_storage_node_properties_update;
use crate::AppState;
use actix_web::{get, post, web};
use chrono::Utc;
use commons::context::controller_request_context::NodeHealth;
use data::dto::controller::StorageResponse;
use data::model::microservice_node_model::MicroserviceNode;

#[get("/storage")]
pub async fn fetch_free_storage(
    _state: web::Data<AppState>,
    _node: MicroserviceNode,
) -> Result<web::Json<StorageResponse>, NodeError> {
    todo!()
}

#[post("/storage")]
pub async fn update_storage_node_properties(
    state: web::Data<AppState>,
    node: MicroserviceNode,
    req: web::Json<UpdateStorageNodeProperties>,
) -> Result<web::Json<UpdateStorageNodeResponse>, NodeError> {
    perform_storage_node_properties_update(req.0, &state.session, node).await?;

    Ok(web::Json(UpdateStorageNodeResponse {}))
}

#[post("/heartbeat")]
pub async fn microservice_heart_beat(
    node: MicroserviceNode,
    state: web::Data<AppState>,
) -> Result<String, NodeError> {
    let mut map = state.req_ctx.node_health.write().await;
    let _ = map.insert(
        node.id,
        NodeHealth {
            last_beat: Utc::now(),
        },
    );
    Ok("".to_string())
}
