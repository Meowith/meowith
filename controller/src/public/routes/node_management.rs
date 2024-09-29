use crate::public::node_management_service::{
    do_create_register_code, do_delete_register_code, do_list_register_codes,
};
use crate::AppState;
use actix_web::{delete, get, post, web, HttpResponse};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::{NodeStatus, NodeStatusResponse, ServiceRegisterCodeDto};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct RegisterCodeCreateRequest {
    pub code: String,
}

#[post("/create")]
pub async fn create_register_code(
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<RegisterCodeCreateRequest>> {
    let code = do_create_register_code(
        RegisterCodeCreateRequest {
            code: Uuid::new_v4().to_string(),
        },
        &state.session,
    )
    .await?;

    Ok(web::Json(RegisterCodeCreateRequest { code }))
}

#[delete("/delete/{id}")]
pub async fn delete_register_code(
    req: web::Path<String>,
    state: web::Data<AppState>,
) -> NodeClientResponse<HttpResponse> {
    do_delete_register_code(req.into_inner(), &state.session).await?;
    Ok(HttpResponse::Ok().finish())
}

#[get("/list")]
pub async fn list_register_codes(
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<Vec<ServiceRegisterCodeDto>>> {
    let codes = do_list_register_codes(&state.session).await?;
    Ok(web::Json(codes))
}

#[get("/status")]
pub async fn status(
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<NodeStatusResponse>> {
    let mut statuses: Vec<NodeStatus> = Vec::new();
    let nodes = state.req_ctx.nodes.read().await;
    let node_health = state.req_ctx.node_health.read().await;

    for node in &*nodes {
        let assoc_health = node_health.get(&node.id);
        let mut status = NodeStatus {
            microservice_type: node.microservice_type,
            id: node.id,
            address: node.address,
            info: None,
            created: node.created,
            last_beat: Default::default(),
            access_token_issued_at: node.access_token_issued_at,
        };

        if let Some(health) = assoc_health {
            status.info = health.info.clone();
            status.last_beat = health.last_beat;
            statuses.push(status);
        }
    }

    Ok(web::Json(NodeStatusResponse { nodes: statuses }))
}
