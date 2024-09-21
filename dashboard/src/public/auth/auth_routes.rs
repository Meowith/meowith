use crate::public::auth::auth_service::do_login;
use crate::public::auth::auth_service::do_register;
use crate::AppState;
use actix_web::{get, post, web, HttpRequest};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::OwnUserInfo;
use data::model::user_model::User;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

#[post("/login/{method}")]
pub async fn login(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<AuthResponse>> {
    let authenticator_method = path.into_inner();

    do_login(req, authenticator_method, &state)
        .await
        .map(web::Json)
}

#[get("/me")]
pub async fn own_user_info(user: User) -> NodeClientResponse<web::Json<OwnUserInfo>> {
    Ok(web::Json(OwnUserInfo::from(user)))
}

#[derive(Serialize)]
pub struct MethodsResponse {
    pub methods: Vec<String>,
}
#[get("/methods")]
pub async fn get_methods(
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<MethodsResponse>> {
    let methods: Vec<String> = state.authentication.keys().cloned().collect();

    Ok(web::Json(MethodsResponse { methods }))
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
    state: web::Data<AppState>,
) -> NodeClientResponse<web::Json<AuthResponse>> {
    do_register(req.0, &state).await.map(web::Json)
}
