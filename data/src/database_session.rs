use scylla::transport::errors::NewSessionError;
use scylla::{CachingSession, Session, SessionBuilder};

pub static CACHE_SIZE: usize = 256;

pub async fn build_session(
    known_nodes: &Vec<String>,
    user: &String,
    password: &String,
    cache_size: usize,
) -> Result<CachingSession, NewSessionError> {
    Ok(CachingSession::from(
        build_raw_session(known_nodes, user, password).await?,
        cache_size,
    ))
}

pub async fn build_raw_session(
    known_nodes: &Vec<String>,
    user: &String,
    password: &String,
) -> Result<Session, NewSessionError> {
    SessionBuilder::new()
        .known_nodes(known_nodes)
        .user(user, password)
        .build()
        .await
}
