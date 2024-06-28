use scylla::transport::errors::NewSessionError;
use scylla::{CachingSession, SessionBuilder};

pub async fn build_session(
    known_nodes: &Vec<String>,
    user: &String,
    password: &String,
    cache_size: usize,
) -> Result<CachingSession, NewSessionError> {
    Ok(CachingSession::from(SessionBuilder::new()
            .known_nodes(known_nodes)
            .user(user, password)
            .build()
            .await?, cache_size))
}
