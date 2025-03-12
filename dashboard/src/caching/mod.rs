use crate::caching::db::VALIDATE_NONCE;
use cached::Cached;

mod db;
mod invalidator;
pub mod mgpp_handler;

/// Clears all caches, to be used upon re-connecting with the control network, as by that time
/// invalidation packets might have been missed.
pub async fn clear_caches() {
    VALIDATE_NONCE.lock().await.cache_clear()
}
