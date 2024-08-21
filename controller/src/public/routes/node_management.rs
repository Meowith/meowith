use crate::public::node_management_service::do_create_register_code;
use crate::AppState;
use actix_web::{post, web};
use commons::error::std_response::NodeClientResponse;
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
