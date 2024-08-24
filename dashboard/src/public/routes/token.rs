use crate::public::service::token_service::do_issue_app_token;
use crate::AppState;
use actix_web::web::Data;
use actix_web::{post, web};
use commons::error::std_response::NodeClientResponse;
use commons::permission::AppTokenPermit;
use data::model::user_model::User;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct TokenIssueRequest {
    pub app_id: Uuid,
    pub name: String,
    pub perms: Vec<AppTokenPermit>,
}

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
