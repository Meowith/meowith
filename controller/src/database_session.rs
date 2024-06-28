#[macro_use]
extern crate lazy_static;

use lazy_static::lazy_static;
use scylla::{CachingSession};
use tokio::sync::RwLock;

lazy_static! {
    static ref SESSION: RwLock<Option<CachingSession>> = RwLock::new(None);
}

pub async fn set_session(session: CachingSession) {
    let mut session_write_lock = SESSION.write().await;

    *session_write_lock = Some(session);
}

pub async fn get_session() -> CachingSession {
    let session_read_lock = SESSION.read().await;

    session_read_lock.clone().unwrap()
}