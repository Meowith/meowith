use crate::framework::error::ProtocolError;
use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;

#[async_trait]
pub trait ProtocolAuthenticator<T>: Send + Sync {
    async fn authenticate(&self, stream: &mut TlsStream<TcpStream>) -> Result<T, ProtocolError>;
}
