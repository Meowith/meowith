use crate::public::routes::token::TokenIssueRequest;
use crate::public::service::{has_app_permission, PermCheckScope};
use crate::AppState;
use actix_web::web::Data;
use chrono::Utc;
use commons::error::std_response::NodeClientResponse;
use commons::permission::AppTokenData;
use data::access::app_access::{get_app_by_id, insert_app_token};
use data::model::app_model::AppToken;
use data::model::user_model::User;
use uuid::Uuid;

pub async fn do_issue_app_token(
    req: TokenIssueRequest,
    app_state: Data<AppState>,
    user: User,
) -> NodeClientResponse<String> {
    let app = get_app_by_id(req.app_id, &app_state.session).await?;

    for perm in &req.perms {
        has_app_permission(
            &user,
            &app,
            perm.allowance,
            &app_state.session,
            PermCheckScope::Buckets,
        )
        .await?
    }

    let nonce = Uuid::new_v4();
    let now = Utc::now();

    let token = AppToken {
        app_id: app.id,
        issuer_id: user.id,
        name: req.name.clone(),
        nonce,
        created: now,
        last_modified: now,
    };
    let token_data = AppTokenData {
        app_id: app.id,
        issuer_id: user.id,
        name: req.name,
        nonce,
        perms: req.perms,
    };

    insert_app_token(&token, &app_state.session).await?;
    let token = app_state.jwt_service.generate_token(&token_data)?;

    Ok(token)
}
