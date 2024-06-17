use scylla::transport::errors::NewSessionError;
use scylla::{CachingSession, SessionBuilder};

pub async fn build_session(
    known_nodes: Vec<&str>,
    cache_size: usize,
) -> Result<CachingSession, NewSessionError> {
    Ok(CachingSession::from(
        SessionBuilder::new()
            .known_nodes(known_nodes)
            .build()
            .await?,
        cache_size,
    ))
}
