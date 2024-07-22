use cached::proc_macro::cached;
use cached::TimedCache;
use scylla::CachingSession;

use commons::access_token_service::ClaimKey;
use commons::permission::AppTokenData;
use data::access::app_access::get_app_token;

#[cached(
    ty = "TimedCache<ClaimKey, bool>",
    create = "{ TimedCache::with_lifespan(60) }",
    convert = r#"{ ClaimKey::from(claims) }"#
)]
pub async fn validate_nonce(claims: &AppTokenData, session: &CachingSession) -> bool {
    let token = get_app_token(
        claims.app_id,
        claims.issuer_id,
        claims.name.clone(),
        session,
    )
    .await;
    match token {
        Ok(token) => token.nonce == claims.nonce,
        Err(_) => false,
    }
}
