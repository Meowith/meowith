use crate::public::service::{
    has_app_permission, PermCheckScope, DELETE_ALL_TOKEN_ALLOWANCE, LIST_ALL_TOKEN_ALLOWANCE,
    NO_ALLOWANCE,
};
use crate::AppState;
use actix_web::web;
use actix_web::web::Data;
use chrono::Utc;
use commons::access_token_service::ClaimKey;
use commons::cache::CacheId;
use commons::error::std_response::NodeClientResponse;
use commons::permission::{AppTokenData, AppTokenPermit};
use data::access::app_access::{
    delete_app_token, get_app_by_id, get_app_token, get_app_tokens, get_app_tokens_by_issuer,
    insert_app_token, AppTokenItem,
};
use data::dto::entity::{
    AppTokenDTO, TokenDeleteRequest, TokenIssueRequest, TokenListRequest, TokenListResponse,
};
use data::error::MeowithDataError;
use data::model::app_model::AppToken;
use data::model::user_model::User;
use futures_util::StreamExt;
use protocol::mgpp::packet::MGPPPacket;
use scylla::client::caching_session::CachingSession;
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
            PermCheckScope::Buckets(perm.bucket_id),
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
        perms: req.perms.into_iter().map(AppTokenPermit::from).collect(),
    };

    insert_app_token(&token, &app_state.session).await?;
    let token = app_state.jwt_service.generate_token(&token_data)?;

    Ok(token)
}

pub async fn do_list_tokens(
    req: TokenListRequest,
    user: User,
    session: &CachingSession,
) -> NodeClientResponse<web::Json<TokenListResponse>> {
    let app = get_app_by_id(req.app_id, session).await?;
    let issuer = Uuid::try_parse(&req.issuer).ok();
    let token_stream = if let Some(issuer) = issuer {
        has_app_permission(
            &user,
            &app,
            *NO_ALLOWANCE,
            session,
            PermCheckScope::Application,
        )
        .await?;

        get_app_tokens_by_issuer(app.id, issuer, session).await?
    } else {
        has_app_permission(
            &user,
            &app,
            *LIST_ALL_TOKEN_ALLOWANCE,
            session,
            PermCheckScope::Application,
        )
        .await?;

        get_app_tokens(app.id, session).await?
    };

    let tokens = token_stream
        .collect::<Vec<AppTokenItem>>()
        .await
        .into_iter()
        .map(|item| item.map(AppTokenDTO::from))
        .collect::<Result<Vec<_>, _>>()
        .map_err(MeowithDataError::from)?;

    Ok(web::Json(TokenListResponse { tokens }))
}

pub async fn do_delete_token(
    req: TokenDeleteRequest,
    user: User,
    state: &AppState,
) -> NodeClientResponse<()> {
    let session = &state.session;

    if req.issuer_id != user.id {
        let app = get_app_by_id(req.app_id, session).await?;
        has_app_permission(
            &user,
            &app,
            *DELETE_ALL_TOKEN_ALLOWANCE,
            session,
            PermCheckScope::Application,
        )
        .await?;
    }

    let token = get_app_token(req.app_id, req.issuer_id, req.name, session).await?;
    delete_app_token(&token, session).await?;

    let cache_id: u8 = CacheId::ValidateNonce.into();

    state
        .mgpp_client
        .write_packet(MGPPPacket::InvalidateCache {
            cache_id: cache_id as u32,
            cache_key: serde_cbor::to_vec(&ClaimKey {
                app_id: token.app_id,
                issuer_id: token.issuer_id,
                name: token.name,
                nonce: token.nonce,
            })
            .unwrap(),
        })
        .await?;

    Ok(())
}
