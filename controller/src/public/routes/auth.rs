use crate::AppState;
use actix_web::{get, post, web, HttpRequest};
use commons::error::std_response::{NodeClientError, NodeClientResponse};
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

pub async fn do_login(
    req: HttpRequest,
    method: String,
    state: &AppState,
) -> NodeClientResponse<AuthResponse> {
    let facade = state.auth.get(&method).ok_or(NodeClientError::BadRequest)?;

    let credentials = facade.convert(&req).map_err(|_| NodeClientError::BadAuth)?;

    let claims = facade
        .authenticate(credentials, &state.session)
        .await
        .map_err(|_| NodeClientError::BadAuth)?;

    let token = state.auth_jwt_service.generate_token(claims)?;

    Ok(AuthResponse { token })
}
