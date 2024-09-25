use crate::public::service::token_service::{do_delete_token, do_issue_app_token, do_list_tokens};
use crate::AppState;
use actix_web::web::Data;
use actix_web::{delete, get, post, web, HttpResponse};
use commons::error::std_response::NodeClientResponse;
use data::dto::entity::{
    TokenDeleteRequest, TokenIssueRequest, TokenListRequest, TokenListResponse,
};
use data::model::user_model::User;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AppTokenResponse {
    pub token: String,
}

#[post("/issue")]
pub async fn issue_app_token(
    req: web::Json<TokenIssueRequest>,
    app_state: Data<AppState>,
    user: User,
) -> NodeClientResponse<web::Json<AppTokenResponse>> {
    let token = do_issue_app_token(req.0, app_state, user).await?;
    Ok(web::Json(AppTokenResponse { token }))
}

#[get("/{app_id}/{issuer}")]
pub async fn list_tokens(
    app_state: Data<AppState>,
    user: User,
    req: web::Path<TokenListRequest>,
) -> NodeClientResponse<web::Json<TokenListResponse>> {
    do_list_tokens(req.into_inner(), user, &app_state.session).await
}

#[delete("/{app_id}/{issuer_id}/{name}")]
pub async fn delete_token(
    app_state: Data<AppState>,
    user: User,
    req: web::Path<TokenDeleteRequest>,
) -> NodeClientResponse<HttpResponse> {
    do_delete_token(req.into_inner(), user, &app_state).await?;
    Ok(HttpResponse::Ok().finish())
}
