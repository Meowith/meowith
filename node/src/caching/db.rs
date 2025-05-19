use crate::caching::invalidator::CacheInvalidator;
use async_trait::async_trait;
use cached::proc_macro::cached;
use cached::{Cached, TimedCache};
use commons::access_token_service::ClaimKey;
use commons::permission::AppTokenData;
use data::access::app_access::get_app_token;
use scylla::client::caching_session::CachingSession;

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

#[derive(Debug)]
pub struct ValidateNonceInvalidator;

#[async_trait]
impl CacheInvalidator for ValidateNonceInvalidator {
    async fn invalidate(&self, cache_key: &[u8]) {
        let claim_key: serde_cbor::error::Result<ClaimKey> = serde_cbor::from_slice(cache_key);
        if let Ok(claim_key) = claim_key {
            VALIDATE_NONCE.lock().await.cache_remove(&claim_key);
        }
    }
}
