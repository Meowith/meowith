use scylla::transport::errors::NewSessionError;
use scylla::{CachingSession, Session, SessionBuilder};

pub static CACHE_SIZE: usize = 256;

pub async fn build_session(
    known_nodes: &Vec<String>,
    user: &String,
    password: &String,
    keyspace: Option<&String>,
    cache_size: usize,
) -> Result<CachingSession, NewSessionError> {
    Ok(CachingSession::from(
        build_raw_session(known_nodes, user, password, keyspace).await?,
        cache_size,
    ))
}

pub async fn build_raw_session(
    known_nodes: &Vec<String>,
    user: &String,
    password: &String,
    keyspace: Option<&String>,
) -> Result<Session, NewSessionError> {
    let mut builder = SessionBuilder::new()
        .known_nodes(known_nodes)
        .user(user, password);

    if let Some(keyspace) = keyspace {
        builder = builder.use_keyspace(keyspace, true);
    }

    builder.build().await
}
