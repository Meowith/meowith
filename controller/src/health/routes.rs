use std::collections::hash_map::Entry;
use std::collections::HashMap;

use actix_web::{get, post, web};
use chrono::Utc;
use commons::cache::CacheId;
use commons::context::controller_request_context::NodeHealth;
use data::dto::controller::{PeerStorage, StorageResponse, UpdateStorageNodeProperties};
use data::model::microservice_node_model::{MicroserviceNode, MicroserviceType};
use log::debug;

use crate::discovery::routes::UpdateStorageNodeResponse;
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
    let nodes = state.req_ctx.nodes.read().await;

    let storage_node_i8: i8 = MicroserviceType::StorageNode.into();
    for node in &*nodes {
        if node.microservice_type == storage_node_i8 {
            peers.insert(
                node.id,
                PeerStorage {
                    storage: 0,
                    addr: node.address,
                },
            );
        }
    }

    for (k, v) in &*node_health {
        if v.available_storage.is_some() {
            match peers.entry(*k) {
                Entry::Occupied(mut node) => node.get_mut().storage = v.available_storage.unwrap(),
                Entry::Vacant(_) => {}
            }
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
    debug!(
        "update_storage_node_properties {:?} for node {node:?}",
        &req.0
    );
    let free_space = req.0.max_space - req.0.used_space;
    let max_space = req.0.max_space;
    perform_storage_node_properties_update(req.0, &state.session, node.clone()).await?;

    let mut map = state.req_ctx.node_health.write().await;
    let _ = map.insert(
        node.id,
        NodeHealth {
            last_beat: Utc::now(),
            available_storage: Some(free_space),
            max_storage: Some(max_space),
        },
    );

    let cache_id: u8 = CacheId::NodeStorageMap.into();
    state
        .catche_server
        .write_invalidate_packet(cache_id as u32, &[])
        .await;

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
                max_storage: entry.get().max_storage,
            });
        }
        Entry::Vacant(entry) => {
            entry.insert(NodeHealth {
                last_beat: Utc::now(),
                available_storage: None,
                max_storage: None,
            });
        }
    }
    Ok("".to_string())
}
