use crate::setup::auth_service::{do_login, do_register};
use crate::setup_procedure::SetupAppState;
use actix_web::{get, post, web, HttpRequest};
use commons::error::std_response::NodeClientResponse;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EmptyResponse;

#[post("/login/{method}")]
pub async fn login(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<SetupAppState>,
) -> NodeClientResponse<web::Json<EmptyResponse>> {
    let authenticator_method = path.into_inner();

    do_login(req, authenticator_method, &state).await?;

    Ok(web::Json(EmptyResponse))
}

#[derive(Deserialize, Serialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

// This method is only accessible when the auth type BASIC is enabled

// Authentication type BASIC is generally deprecated because it is not that secure

#[post("/register")]
pub async fn register(
    req: web::Json<RegisterRequest>,
    state: web::Data<SetupAppState>,
) -> NodeClientResponse<web::Json<EmptyResponse>> {
    do_register(req.0, &state).await?;

    Ok(web::Json(EmptyResponse))
}

#[derive(Serialize)]
pub struct MethodsResponse {
    pub methods: Vec<String>,
}

#[get("/methods")]
pub async fn get_methods(
    state: web::Data<SetupAppState>,
) -> NodeClientResponse<web::Json<MethodsResponse>> {
    let methods: Vec<String> = state.auth.keys().cloned().collect();

    Ok(web::Json(MethodsResponse { methods }))
}
