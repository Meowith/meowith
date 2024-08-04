use std::collections::hash_map::Entry;
use std::collections::HashMap;

use actix_web::{get, post, web};
use chrono::Utc;

use commons::context::controller_request_context::NodeHealth;
use data::dto::controller::StorageResponse;
use data::model::microservice_node_model::MicroserviceNode;

use crate::discovery::routes::{UpdateStorageNodeProperties, UpdateStorageNodeResponse};
use crate::error::node::NodeError;
use crate::health::health_service::perform_storage_node_properties_update;
use crate::AppState;

#[get("/storage")]
pub async fn fetch_free_storage(
    state: web::Data<AppState>,
    _node: MicroserviceNode,
) -> web::Json<StorageResponse> {
    let mut peers = HashMap::new();

    let node_health = state.req_ctx.node_health.read().await;
    for (k, v) in &*node_health {
        if v.available_storage.is_some() {
            peers.insert(*k, v.available_storage.unwrap());
        }
    }

    web::Json(StorageResponse { peers })
}

#[post("/storage")]
pub async fn update_storage_node_properties(
    state: web::Data<AppState>,
    node: MicroserviceNode,
    req: web::Json<UpdateStorageNodeProperties>,
) -> Result<web::Json<UpdateStorageNodeResponse>, NodeError> {
    let free_space = req.0.max_space - req.0.used_space;
    perform_storage_node_properties_update(req.0, &state.session, node.clone()).await?;

    let mut map = state.req_ctx.node_health.write().await;
    let _ = map.insert(
        node.id,
        NodeHealth {
            last_beat: Utc::now(),
            available_storage: Some(free_space),
        },
    );

    Ok(web::Json(UpdateStorageNodeResponse {}))
}

#[post("/heartbeat")]
pub async fn microservice_heart_beat(
    node: MicroserviceNode,
    state: web::Data<AppState>,
) -> Result<String, NodeError> {
    let mut map = state.req_ctx.node_health.write().await;
    match map.entry(node.id) {
        Entry::Occupied(mut entry) => {
            entry.insert(NodeHealth {
                last_beat: Utc::now(),
                available_storage: entry.get().available_storage,
            });
        }
        Entry::Vacant(entry) => {
            entry.insert(NodeHealth {
                last_beat: Utc::now(),
                available_storage: None,
            });
        }
    }
    Ok("".to_string())
}
